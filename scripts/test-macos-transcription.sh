#!/bin/sh
# Opt-in integration: uses macOS speech services and synthetic audio only.
# Set TARANTINO_INSTALL_SPEECH_MODEL=1 to allow English model setup if needed.
set -eu
cd "$(dirname "$0")/.."
transcription_tmp=$(mktemp -d "${TMPDIR:-/tmp}/tarantino-speech-test.XXXXXX")
trap 'rm -rf "$transcription_tmp"' EXIT HUP INT TERM
xcrun swiftc -parse-as-library -O -module-cache-path "$transcription_tmp/modules" \
  src-tauri/src/transcription/SpeechHelper.swift -o "$transcription_tmp/speech"
if [ "${TARANTINO_INSTALL_SPEECH_MODEL:-0}" = 1 ]; then
  "$transcription_tmp/speech" install en-US
fi
"$transcription_tmp/speech" status en-US > "$transcription_tmp/status.json"
python3 - "$transcription_tmp/status.json" <<'PY'
import json,sys
status=json.load(open(sys.argv[1]))
assert status['available'] and status.get('modelStatus')=='installed', 'English speech model must be installed before this test'
PY
say -o "$transcription_tmp/speech.aiff" 'Make something worth watching. Smooth motion makes the recording easy to follow.'
"$transcription_tmp/speech" transcribe "$transcription_tmp/speech.aiff" en-US > "$transcription_tmp/result.json"
python3 - "$transcription_tmp/result.json" <<'PY'
import json,math,re,sys
result=json.load(open(sys.argv[1])); words=result['words']
actual=' '.join(re.sub(r'[^a-z ]','',w['text'].lower()) for w in words)
assert actual=='make something worth watching smooth motion makes the recording easy to follow',actual
assert len(words)==12 and len({w['id'] for w in words})==12
previous=0
for word in words:
    start,end=word['startMs'],word['endMs']
    assert math.isfinite(start) and math.isfinite(end) and 0<=start<end<=6000
    assert start>=previous and word['type']=='word'
    previous=start
print(f"Local transcription passed: {len(words)} words with individual source timestamps")
PY
if "$transcription_tmp/speech" transcribe "$transcription_tmp/missing.wav" en-US > "$transcription_tmp/error.json"; then
  echo 'Missing audio unexpectedly succeeded' >&2
  exit 1
fi
python3 - "$transcription_tmp/error.json" <<'PY'
import json,sys
assert isinstance(json.load(open(sys.argv[1]))['error'],str)
PY
