#!/usr/bin/env python3
"""Complete Turkish help and historical changelog. Public software texts only."""
from concurrent.futures import ThreadPoolExecutor,as_completed
from add_turkish_translation import ROOT, translate_batch, build_batches
FILES=[("guida.txt","guida_tr.txt"),("CHANGELOG_IT.md","CHANGELOG_TR.md")]
def run():
    rows={}
    srcs={}
    for source,dest in FILES:
        lines=(ROOT/source).read_text(encoding="utf-8").splitlines()
        srcs[source]=(dest,lines)
        for line in lines:
            v=line.strip()
            if v and not v.startswith("~~~") and v not in ("---","***"):
                rows[v]=None
    unique=list(rows)
    groups=build_batches(unique)
    print(f"DOC_LINES={len(unique)} GROUPS={len(groups)}",flush=True)
    done=0
    with ThreadPoolExecutor(max_workers=6) as pool:
        jobs={pool.submit(translate_batch,part):part for part in groups}
        for f in as_completed(jobs):
            part=jobs[f]
            result=f.result()
            rows.update(zip(part,result))
            done+=1
            if done%20==0 or done==len(groups):
                print(f"PROGRESS {done}/{len(groups)}",flush=True)
    for source,(target,original) in srcs.items():
        translated=[]
        for line in original:
            v=line.strip()
            if v in rows and rows[v] is not None:
                lead=line[:len(line)-len(line.lstrip())]
                translated.append(lead+rows[v])
            else:translated.append(line)
        (ROOT/target).write_text("\n".join(translated)+"\n",encoding="utf-8")
        print("WRITTEN",target,len(translated),flush=True)
if __name__=="__main__":run()
