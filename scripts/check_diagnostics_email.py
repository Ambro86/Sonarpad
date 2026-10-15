"""Exercise the PHP email endpoint locally with a fake PHPMailer (no email is sent)."""
import argparse
import base64
import io
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import zipfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--php-source', required=True, type=Path)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='sonarpad-report-test-') as folder:
        root = Path(folder)
        source = args.php_source.read_text(encoding='utf-8')
        # The test fixture must never contain the real SMTP credentials.
        source = re.sub(r'^\$smtp\w+\s*=.*?;', "$smtpConfigUnused = '';", source, flags=re.M)
        source = source.replace("$receiverEmail =", "$smtpHost='localhost'; $smtpPort=465; $smtpUsername='test@example.invalid'; $smtpPassword='fake';\n$receiverEmail =")
        (root / 'logdeveloper.php').write_text(source, encoding='utf-8')
        library = root / 'phpmailer' / 'src'
        library.mkdir(parents=True)
        for file in ['Exception.php', 'SMTP.php']:
            (library / file).write_text('<?php', encoding='utf-8')
        (library / 'PHPMailer.php').write_text(r'''<?php
namespace PHPMailer\PHPMailer;
#[\AllowDynamicProperties]
class PHPMailer {
    const ENCRYPTION_SMTPS = 'ssl';
    public array $attachments = [];
    public function __construct($exceptions = true) {}
    public function isSMTP() {}
    public function setFrom($address, $name) {}
    public function addAddress($address) { $this->recipient = $address; }
    public function addReplyTo($address) { $this->replyTo = $address; }
    public function isHTML($value) {}
    public function addStringAttachment($data, $name, $encoding, $type) {
        $this->attachments[] = ['name'=>$name, 'data'=>base64_encode($data), 'type'=>$type];
    }
    public function send() {
        if (str_contains($this->Body, 'SIMULATE_SMTP_FAILURE')) { throw new \RuntimeException('simulated'); }
        file_put_contents(getenv('SONARPAD_TEST_MAIL'), json_encode([
            'replyTo'=>$this->replyTo ?? '', 'recipient'=>$this->recipient, 'body'=>$this->Body, 'subject'=>$this->Subject,
            'attachments'=>$this->attachments]));
        return true;
    }
}
''', encoding='utf-8')
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0))
            port = sock.getsockname()[1]
        capture = root / 'mail.json'
        env = os.environ.copy()
        env['SONARPAD_TEST_MAIL'] = str(capture)
        with (root / 'server.log').open('wb') as log:
            process = subprocess.Popen(['php', '-S', f'127.0.0.1:{port}', '-t', str(root)],
                                       env=env, stdout=log, stderr=log,
                                       creationflags=subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0)
            try:
                url = f'http://127.0.0.1:{port}/logdeveloper.php'
                def request(payload=None, suffix=''):
                    data = None if payload is None else json.dumps(payload).encode('utf-8')
                    req = urllib.request.Request(url + suffix, data=data, headers={'Content-Type':'application/json'})
                    try:
                        with urllib.request.urlopen(req, timeout=5) as response:
                            return response.status, json.load(response)
                    except urllib.error.HTTPError as error:
                        return error.code, json.load(error)
                for _ in range(50):
                    try:
                        status, result = request(suffix='?capabilities=1')
                        break
                    except OSError:
                        time.sleep(.1)
                else:
                    raise AssertionError('Local PHP server did not start')
                assert status == 200 and result['email_zip_v1'] is True
                assert not capture.exists()
                assert request()[0] == 405
                assert request({'report':' '})[0] == 400
                assert request({'report':['bad type']})[0] == 400
                assert request({'report':'x' * 32769})[0] == 413
                assert request({'report':'test', 'diagnostics_zip_base64':'bad zip'})[0] == 400
                assert request({'report':'test', 'email':'bad\r\nBcc: other@example.org'})[0] == 400
                assert not capture.exists()
                status, result = request({'name':'', 'report':'Mobile test', 'log':'sample mobile log'})
                assert status == 200 and result['ok'] and not result['attachment_received']
                mail = json.loads(capture.read_text())
                assert mail['recipient'] == 'ambro86@gmail.com'
                assert mail['replyTo'] == ''
                assert 'Anonimo' in mail['body'] and 'Mobile' in mail['subject']
                assert base64.b64decode(mail['attachments'][0]['data']) == b'sample mobile log'
                # Regression: iPhone keeps about 8 MiB, and a final entry may exceed that.
                for size in [4446310, 8 * 1024 * 1024 + 1024]:
                    large_log = 'L' * size
                    status, result = request({'report':'Large iPhone log', 'log':large_log})
                    assert status == 200 and result['ok'], f'Log of {size} bytes rejected: {status}'
                    mail = json.loads(capture.read_text())
                    assert base64.b64decode(mail['attachments'][0]['data']) == large_log.encode()
                assert request({'report':'Too large', 'log':'L' * (10 * 1024 * 1024 + 1)})[0] == 413
                archive = io.BytesIO()
                with zipfile.ZipFile(archive, 'w') as z:
                    z.writestr('version.txt', 'synthetic diagnostics')
                status, result = request({'name':'Test È', 'report':'Windows test', 'platform':'windows', 'email':'reply@example.org',
                    'log':'version', 'diagnostics_zip_base64':base64.b64encode(archive.getvalue()).decode()})
                assert status == 200 and result['delivery'] == 'email' and result['attachment_received']
                mail = json.loads(capture.read_text())
                assert mail['replyTo'] == 'reply@example.org'
                assert 'reply@example.org' in mail['body']
                assert 'Test È' in mail['body'] and 'Windows' in mail['subject']
                attachment = next(a for a in mail['attachments'] if a['name'].endswith('.zip'))
                assert base64.b64decode(attachment['data']) == archive.getvalue()
                previous = capture.read_bytes()
                assert request({'report':'SIMULATE_SMTP_FAILURE'}) == (502, {'ok':False})
                assert capture.read_bytes() == previous
                assert not (root / 'data_private').exists()
                print('PHP: 13 scenarios passed; synthetic email only, no real messages sent.')
            finally:
                process.terminate()
                process.wait(timeout=5)


if __name__ == '__main__':
    main()
