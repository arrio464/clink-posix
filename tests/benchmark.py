"""Randomized warm-launch benchmark. Standard library only. Windows process counters.

Wall time includes child work; CPU/RSS/IO counters are executor-only, NOT process-tree totals.
Input/output use NUL. Pipe/substitution workloads still transfer real bytes internally.
"""
import ctypes as C
from ctypes import wintypes as W
import argparse, hashlib, json, math, os, platform, random, statistics, subprocess, sys, time
from pathlib import Path
from support import ROOT, WORK, EXE, FIXTURE, MANIFEST, native_args, prepare_fixture

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--samples',type=int,default=25)
parser.add_argument('--warmups',type=int,default=3)
parser.add_argument('--output',type=Path,default=WORK/'benchmark.json')
parser.add_argument('--python-reference',type=Path)
parser.add_argument('--optimized-reference',type=Path)
options=parser.parse_args()
if options.samples<1 or options.warmups<0:parser.error('samples must be positive; warmups must be non-negative')
for path in (options.python_reference,options.optimized_reference):
    if path and not path.is_file():parser.error('Reference helper does not exist')
prepare_fixture()
class Memory(C.Structure):
    _fields_=[('cb',W.DWORD),('PageFaultCount',W.DWORD)]+[(s,C.c_size_t) for s in ['PeakWorkingSetSize','WorkingSetSize','QuotaPeakPagedPoolUsage','QuotaPagedPoolUsage','QuotaPeakNonPagedPoolUsage','QuotaNonPagedPoolUsage','PagefileUsage','PeakPagefileUsage']]
class IO(C.Structure):
    _fields_=[(s,C.c_ulonglong) for s in ['ReadOperationCount','WriteOperationCount','OtherOperationCount','ReadTransferCount','WriteTransferCount','OtherTransferCount']]
k=C.WinDLL('kernel32',use_last_error=True);p=C.WinDLL('psapi',use_last_error=True)
p.GetProcessMemoryInfo.argtypes=[W.HANDLE,C.POINTER(Memory),W.DWORD];p.GetProcessMemoryInfo.restype=W.BOOL
k.GetProcessTimes.argtypes=[W.HANDLE]+[C.POINTER(W.FILETIME)]*4;k.GetProcessTimes.restype=W.BOOL
k.GetProcessIoCounters.argtypes=[W.HANDLE,C.POINTER(IO)];k.GetProcessIoCounters.restype=W.BOOL
def required(ok):
    if not ok:raise C.WinError(C.get_last_error())
def tick(v):return (v.dwHighDateTime<<32)|v.dwLowDateTime
def once(args):
    start=time.perf_counter_ns()
    with subprocess.Popen(args,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE) as proc:
        _,stderr=proc.communicate(timeout=20)
        elapsed=(time.perf_counter_ns()-start)/1e6
        if proc.returncode:raise RuntimeError((args,proc.returncode,stderr))
        mem=Memory();mem.cb=C.sizeof(mem)
        required(p.GetProcessMemoryInfo(int(proc._handle),C.byref(mem),mem.cb))
        birth,death,kernel,user=[W.FILETIME() for _ in range(4)]
        required(k.GetProcessTimes(int(proc._handle),*[C.byref(v) for v in [birth,death,kernel,user]]))
        io=IO();required(k.GetProcessIoCounters(int(proc._handle),C.byref(io)))
    return dict(wall_ms=elapsed,cpu_ms=(tick(kernel)+tick(user))/10000,peak_working_set_mib=mem.PeakWorkingSetSize/1048576,peak_commit_mib=mem.PeakPagefileUsage/1048576,read_bytes=io.ReadTransferCount,write_bytes=io.WriteTransferCount)
f="'"+FIXTURE.as_posix()+"'"
workloads={
    'builtin_echo':'echo hello',
    'native_sort':f+' sort',
    'two_stage_pipeline':f+" printf '%s\\n' b a | "+f+' sort',
    'command_substitution':'echo "$(echo hello)"',
    'process_substitution':f+" diff <("+f+" printf 'same\\n') <("+f+" printf 'same\\n')",
    '4MiB_binary_pipeline':f+' emit 4194304 | '+f+' count',
}
backends={'rust_release':native_args()}
hashes={'rust_executable':EXE,'fixture':FIXTURE}
if options.python_reference:
    backends['python_reference']=[sys.executable,str(options.python_reference)]
    hashes['python_reference']=options.python_reference
if options.optimized_reference:
    backends['python_optimized_S']=[sys.executable,'-S',str(options.optimized_reference)]
    hashes['python_optimized_S']=options.optimized_reference
commands={f'{b}/{w}':prefix+['-c',line] for b,prefix in backends.items() for w,line in workloads.items()}
commands['direct_fixture_sort']=[str(FIXTURE),'sort']
commands['python_empty']=[sys.executable,'-c','pass']
commands['rust_version']=[str(EXE),'--version']
raw={name:[] for name in commands};rng=random.Random(20261011)
for warm in range(options.warmups):
    for args in commands.values():once(args)
for round_index in range(options.samples):
    keys=list(commands);rng.shuffle(keys)
    for name in keys:raw[name].append(once(commands[name]))
    if round_index%5==4:print(f'{round_index+1}/{options.samples} rounds',flush=True)
results={}
for name,rows in raw.items():
    wall=sorted(r['wall_ms'] for r in rows)
    results[name]={'median_ms':round(statistics.median(wall),3),'p95_ms':round(wall[math.ceil(len(wall)*.95)-1],3),'min_ms':round(min(wall),3)}
    for key in rows[0]:
        if key!='wall_ms':results[name]['median_'+key]=round(statistics.median(r[key] for r in rows),3)
data={'date':time.strftime('%Y-%m-%d %H:%M:%S %z'),'platform':platform.platform(),'processor':os.environ.get('PROCESSOR_IDENTIFIER'),'logical_processors':os.cpu_count(),'python':sys.version,'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),'warmups':options.warmups,'samples_per_case':options.samples,'random_seed':20261011,'workload_profile':'native fixtures; not the original uutils/BusyBox measurement','method':'warm launches, randomized case order, NUL stdin/stdout, executor-only CPU/memory/IO counters; latency includes descendants; no Clink adapter or parent-state synchronization', 'binary_bytes':EXE.stat().st_size,'sha256':{label:hashlib.sha256(path.read_bytes()).hexdigest() for label,path in hashes.items()},'results':results,'raw':raw}
options.output.parent.mkdir(parents=True,exist_ok=True)
options.output.write_text(json.dumps(data,ensure_ascii=False,indent=2),encoding='utf8')
print(json.dumps({'binary_bytes':data['binary_bytes'],'results':results},indent=2))
