"""Sample a recorder process tree: python3 script.py PID OUTPUT.json [SECONDS]."""
import json
import statistics
import subprocess
import sys
import time

root_pid, output = int(sys.argv[1]), sys.argv[2]
duration = int(sys.argv[3]) if len(sys.argv) > 3 else 30


def snapshot():
    rows = subprocess.check_output(
        ['ps', '-axo', 'pid=,ppid=,time=,rss='], text=True
    ).splitlines()
    processes = {}
    for row in rows:
        pid, parent, cpu_time, rss = row.split()
        parts = cpu_time.split(':')
        seconds = sum(float(value) * 60 ** index for index, value in enumerate(reversed(parts)))
        processes[int(pid)] = (int(parent), seconds, int(rss) / 1024)
    selected = {root_pid}
    while True:
        descendants = {pid for pid, (parent, _, _) in processes.items() if parent in selected}
        if descendants <= selected:
            break
        selected |= descendants
    return {pid: processes[pid] for pid in selected if pid in processes}


samples = []
previous = snapshot()
start = previous_time = time.monotonic()
for index in range(duration):
    time.sleep(max(0, start + index + 1 - time.monotonic()))
    current = snapshot()
    now = time.monotonic()
    cpu_delta = sum(max(0, values[1] - previous.get(pid, (0, values[1], 0))[1])
                    for pid, values in current.items())
    samples.append({'elapsed': now - start, 'uptime': now,
                    'recorder_cpu_percent': cpu_delta / (now - previous_time) * 100,
                    'recorder_rss_mib': sum(values[2] for values in current.values()),
                    'process_count': len(current)})
    previous, previous_time = current, now
result = {'pid': root_pid, 'resource_samples': samples,
          'mean_resources': {key: statistics.mean(row[key] for row in samples)
                             for key in ['recorder_cpu_percent', 'recorder_rss_mib']},
          'limitations': ['CPU percent is relative to one logical core.',
                          'RSS sums the recorder and descendants; shared pages may be counted more than once.',
                          'Processes which start and exit entirely between samples are not counted.',
                          'GPU utilization is not sampled.']}
with open(output, 'w') as file:
    json.dump(result, file, indent=2)
    file.write('\n')
print(json.dumps(result['mean_resources']))
