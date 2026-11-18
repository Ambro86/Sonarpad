#!/usr/bin/env python3
"""Generate Turkish Sonarpad UI, feature, and calendar catalogs from Italian sources.

Batch Google translations, preserve placeholders and accelerators and verify integrity.
Only writes generated catalogs after translation has completed and been validated.
"""
import json
import re
import time
import random
import threading
from collections import OrderedDict
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
import requests

ROOT = Path(__file__).resolve().parents[1]
IT = json.loads((ROOT / "i18n/it.json").read_text(encoding="utf-8"))
CAL = json.loads((ROOT / "src/app_windows/calendar_data.json").read_text(encoding="utf-8"))
FEATURES = {
    "i18n/features/treccani_it.json": "i18n/features/treccani_tr.json",
    "i18n/features/tv/it.json": "i18n/features/tv/tr.json",
    "i18n/features/la7_play/it.json": "i18n/features/la7_play/tr.json",
}
SEP = "\n<<<SONARPAD_SPLIT_QA7>>>\n"
TOKEN = re.compile(r"\{[A-Za-z_][\w.:+-]*\}|%[0-9.]*[sdf]|\\[nt]|https?://[^\s<>\"']+|<[^<>]+>")
TRANSLATE_URL = "https://translate.googleapis.com/translate_a/single"
thread_local = threading.local()
_failures = []
_cache = {}
_lock = threading.Lock()

def session():
    if not hasattr(thread_local, "session"):
        s = requests.Session()
        s.headers["User-Agent"] = "Mozilla/5.0 SonarpadLocalization/1.0"
        thread_local.session = s
    return thread_local.session

def protect(text):
    saved = []
    def repl(match):
        idx = len(saved)
        saved.append(match.group())
        return f"ZXQ{idx:03d}ZX"
    return TOKEN.sub(repl,text), saved

def restore(text, saved):
    for idx, original in enumerate(saved):
        key=f"ZXQ{idx:03d}ZX"
        # API sometimes inserts spaces around protected tokens.
        text=re.sub(rf"ZXQ\s*{idx:03d}\s*ZX",lambda _:original,text,flags=re.I)
        if key in text:
            text=text.replace(key,original)
    return text

def request_translation(source, attempts=5):
    for i in range(attempts):
        try:
            response=session().get(TRANSLATE_URL,params={"client":"gtx","sl":"it","tl":"tr","dt":"t","q":source},timeout=28)
            if response.status_code==429:
                time.sleep(2+i*3+random.random()); continue
            response.raise_for_status()
            data=response.json()
            return "".join(a[0] for a in data[0] if a and a[0] is not None)
        except (requests.RequestException, ValueError, KeyError, TypeError, IndexError):
            time.sleep(0.7*(i+1)+random.random())
    return None

def translate_batch(items):
    prepared = []
    originals=[]
    for item in items:
        p,arr=protect(item)
        prepared.append(p)
        originals.append(arr)
    translated = request_translation(SEP.join(prepared))
    if translated is not None:
        parts=translated.split(SEP)
        if len(parts)==len(items):
            result=[]
            good=True
            for source,part,saved in zip(items,parts,originals):
                output=restore(part.strip(),saved)
                if any(orig not in output for orig in saved):
                    good=False;break
                if source.count("\t") and "\t" not in output:
                    # Keep accelerator tab separating the shortcut.
                    match=re.search(r"(\t[^\t]+)$",source)
                    if match and not output.endswith(match.group(1)):
                        output=output.replace(match.group(1).strip(),"").strip()+match.group(1)
                result.append(output)
            if good:
                return result
    result=[]
    for source,p,saved in zip(items,prepared,originals):
        translated=request_translation(p)
        if translated is None:
            with _lock:_failures.append(("api",source[:120]))
            result.append(source);continue
        output=restore(translated.strip(),saved)
        if any(orig not in output for orig in saved):
            with _lock:_failures.append(("placeholder",source[:120]))
            result.append(source);continue
        if source.count("\t") and "\t" not in output:
            match=re.search(r"(\t[^\t]+)$",source)
            if match:
                output=output.replace(match.group(1).strip(),"").strip()+match.group(1)
        result.append(output)
    return result

def collect():
    entries=[]
    entries.extend(IT.values())
    for row in CAL["saints"].values():entries.append(row.get("it",row.get("en","")))
    entries.extend(CAL["quotes"]["it"])
    for src in FEATURES:
        obj=json.loads((ROOT/src).read_text(encoding="utf-8"))
        entries.extend(obj.values())
    result=list(dict.fromkeys(v for v in entries if isinstance(v,str) and v.strip()))
    return result

def build_batches(values):
    batches=[];batch=[];length=0
    for value in values:
        # 1800 chars per request to avoid proxy length and truncation
        if batch and (len(batch)>=12 or length+len(value)>1750):
            batches.append(batch);batch=[];length=0
        batch.append(value);length+=len(value)+len(SEP)
    if batch:batches.append(batch)
    return batches

def write_json(path,obj):
    dest=ROOT/path;dest.parent.mkdir(parents=True,exist_ok=True)
    dest.write_text(json.dumps(obj,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")

def main():
    values=collect()
    print(f"TOTAL DISTINCT={len(values)}",flush=True)
    batches=build_batches(values)
    print(f"BATCHES={len(batches)}",flush=True)
    completed=0
    with ThreadPoolExecutor(max_workers=6) as pool:
        jobs={pool.submit(translate_batch,b):b for b in batches}
        for future in as_completed(jobs):
            source=jobs[future]
            try: translated=future.result()
            except Exception as err:
                print("ERROR",str(err)[:180],flush=True)
                translated=source
                _failures.extend(("exception",x[:100]) for x in source)
            with _lock:
                _cache.update(zip(source,translated))
                completed+=1
                if completed%15==0 or completed==len(batches):
                    print(f"PROGRESS {completed}/{len(batches)} translated={len(_cache)} failures={len(_failures)}",flush=True)
    if len(_cache)!=len(values):
        raise RuntimeError(f"Missing translations: {len(values)-len(_cache)}")
    if len(_failures)>max(12,len(values)//40):
        raise RuntimeError(f"Too many failed translation requests ({len(_failures)})")
    tr={k:_cache.get(v,v) for k,v in IT.items()}
    if set(tr)!=set(IT):raise RuntimeError("Key mismatch")
    for k,v in IT.items():
        placeholders=set(TOKEN.findall(v))
        if not placeholders.issubset(set(TOKEN.findall(tr[k]))):
            raise RuntimeError(f"Placeholder mismatch {k}")
    write_json("i18n/tr.json",tr)
    for src,dst in FEATURES.items():
        src_obj=json.loads((ROOT/src).read_text(encoding="utf-8"))
        write_json(dst,{k:_cache.get(v,v) if isinstance(v,str) else v for k,v in src_obj.items()})
    for row in CAL["saints"].values():row["tr"]=_cache.get(row.get("it",""),row.get("it",""))
    CAL["quotes"]["tr"]=[_cache.get(v,v) for v in CAL["quotes"]["it"]]
    write_json("src/app_windows/calendar_data.json",CAL)
    print(f"FINISHED UI={len(tr)} SAINTS={len(CAL['saints'])} QUOTES={len(CAL['quotes']['tr'])} FAILURES={len(_failures)}",flush=True)
    if _failures:print("FAILED_SAMPLES",repr(_failures[:24]),flush=True)

if __name__=="__main__":
    main()
