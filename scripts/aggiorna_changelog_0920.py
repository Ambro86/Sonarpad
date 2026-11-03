"""Allinea Cargo.lock e i changelog localizzati alla versione 0.9.20.
Preserva testo preesistente, terminazioni di riga e codifica UTF-8.
"""
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent.parent
OLD = 'Version 0.9.19 – 2026-10-24'
NEW = 'Version 0.9.20 – 2026-10-25'
NOTES = {
    'CHANGELOG.md': '1. Updated the user guides with detailed instructions for Cut media file and SonarMusic, including keyboard shortcuts, screen-reader navigation, advanced editing, search and favorites.',
    'CHANGELOG_IT.md': '1. Aggiornate le guide con istruzioni dettagliate per Taglia file media e SonarMusic, inclusi i comandi da tastiera, la navigazione con lettori di schermo, il taglio avanzato, la ricerca e i preferiti.',
    'CHANGELOG_CS.md': '1. Aktualizovány uživatelské příručky: podrobné návody ke střihu multimediálních souborů a SonarMusic, včetně klávesových zkratek, ovládání odečítačem obrazovky, pokročilých úprav, vyhledávání a oblíbených položek.',
    'CHANGELOG_DE.md': '1. Die Handbücher wurden mit ausführlichen Anleitungen zu Mediendatei schneiden und SonarMusic aktualisiert, einschließlich Tastenkombinationen, Screenreader-Bedienung, erweitertem Schnitt, Suche und Favoriten.',
    'CHANGELOG_ES.md': '1. Actualizadas las guías con instrucciones detalladas para Cortar archivo multimedia y SonarMusic, incluidos atajos de teclado, navegación con lectores de pantalla, edición avanzada, búsquedas y favoritos.',
    'CHANGELOG_FR.md': '1. Guides mises à jour avec des instructions détaillées pour Découper un fichier multimédia et SonarMusic : raccourcis clavier, navigation avec lecteur d’écran, découpe avancée, recherche et favoris.',
    'CHANGELOG_ID.md': '1. Panduan pengguna diperbarui dengan petunjuk terperinci untuk Potong file media dan SonarMusic, termasuk pintasan keyboard, navigasi pembaca layar, penyuntingan lanjutan, pencarian, dan favorit.',
    'CHANGELOG_LT.md': '1. Atnaujinti naudotojo vadovai su išsamiomis medijos failų kirpimo ir SonarMusic instrukcijomis, įskaitant sparčiuosius klavišus, ekrano skaitytuvo valdymą, išplėstinį redagavimą, paiešką ir mėgstamus elementus.',
    'CHANGELOG_PL.md': '1. Zaktualizowano instrukcje obsługi: szczegółowe opisy przycinania plików multimedialnych i SonarMusic, w tym skróty klawiaturowe, nawigację z czytnikiem ekranu, zaawansowaną edycję, wyszukiwanie i ulubione.',
    'CHANGELOG_PT.md': '1. Guias atualizados com instruções detalhadas para Cortar ficheiro multimédia e SonarMusic, incluindo atalhos de teclado, navegação com leitores de ecrã, edição avançada, pesquisa e favoritos.',
    'CHANGELOG_PT_BR.md': '1. Guias atualizados com instruções detalhadas para Cortar arquivo de mídia e SonarMusic, incluindo atalhos de teclado, navegação com leitores de tela, edição avançada, pesquisa e favoritos.',
    'CHANGELOG_RU.md': '1. Обновлены руководства с подробными инструкциями по обрезке медиафайлов и SonarMusic, включая сочетания клавиш, навигацию с программами экранного доступа, расширенное редактирование, поиск и избранное.',
    'CHANGELOG_SR.md': '1. Ažurirana su uputstva sa detaljnim opisom sečenja medijskih datoteka i funkcije SonarMusic, uključujući prečice na tastaturi, navigaciju čitačem ekrana, napredno uređivanje, pretragu i omiljene stavke.',
    'CHANGELOG_SV.md': '1. Guiderna har uppdaterats med detaljerade instruktioner för att klippa mediefiler och använda SonarMusic, inklusive kortkommandon, skärmläsarnavigering, avancerad redigering, sökning och favoriter.',
    'CHANGELOG_UK.md': '1. Оновлено посібники з докладними інструкціями щодо обрізання медіафайлів і SonarMusic, зокрема комбінаціями клавіш, навігацією з програмами екранного доступу, розширеним редагуванням, пошуком та обраним.',
    'CHANGELOG_VI.md': '1. Đã cập nhật hướng dẫn sử dụng với chỉ dẫn chi tiết về Cắt tệp đa phương tiện và SonarMusic, gồm phím tắt, điều hướng bằng trình đọc màn hình, chỉnh sửa nâng cao, tìm kiếm và mục yêu thích.',
    'CHANGELOG_ZH.md': '1. 已更新用户指南，详细介绍媒体文件剪辑和 SonarMusic 的操作，包括键盘快捷键、屏幕阅读器导航、高级编辑、搜索和收藏功能。',
    'CHANGELOG-HI.txt': '1. मीडिया फ़ाइल काटने और SonarMusic के लिए विस्तृत निर्देशों से सभी गाइड अपडेट की गई हैं। इनमें कीबोर्ड शॉर्टकट, स्क्रीन रीडर नेविगेशन, उन्नत संपादन, खोज और पसंदीदा शामिल हैं।',
}


def main():
    files = {p.name for p in ROOT.glob('CHANGELOG*') if p.is_file()}
    if files != set(NOTES):
        raise RuntimeError(f'Elenco changelog non corrispondente: {sorted(files ^ set(NOTES))}')
    toml = (ROOT / 'Cargo.toml').read_text(encoding='utf-8')
    if not re.search(r'(?m)^version = "0\.9\.20"\s*$', toml):
        raise RuntimeError('Cargo.toml non ha la nuova versione 0.9.20')
    changes = {}
    lock_path = ROOT / 'Cargo.lock'
    lock = lock_path.read_bytes()
    lock_eol = b'\r\n' if b'\r\n' in lock else b'\n'
    old_package = b'name = "sonarpad"' + lock_eol + b'version = "0.9.19"'
    new_package = b'name = "sonarpad"' + lock_eol + b'version = "0.9.20"'
    if lock.count(old_package) != 1:
        raise RuntimeError('Versione del pacchetto Sonarpad non trovata una sola volta in Cargo.lock')
    changes[lock_path] = lock.replace(old_package, new_package, 1)
    for filename, note in NOTES.items():
        path = ROOT / filename
        original = path.read_bytes()
        old_bytes = OLD.encode('utf-8')
        new_bytes = NEW.encode('utf-8')
        if original.count(old_bytes) != 1 or new_bytes in original:
            raise RuntimeError(f'Intestazione versione inattesa in {filename}')
        eol = b'\r\n' if b'\r\n' in original else b'\n'
        prefix = new_bytes + eol + eol + note.encode('utf-8') + eol + eol
        changes[path] = original.replace(old_bytes, prefix + old_bytes, 1)
    for path, content in changes.items():
        temporary = path.with_name(path.name + '.version-update.tmp')
        temporary.write_bytes(content)
        temporary.replace(path)
        print(f'Aggiornato: {path.name}')
    print(f'Versione 0.9.20: aggiornati {len(NOTES)} changelog e Cargo.lock')


if __name__ == '__main__':
    main()
