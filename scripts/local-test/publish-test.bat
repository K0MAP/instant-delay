@echo off
rem Publish a synthetic test stream to InstantDelay's RTMP ingest.
rem The burned-in wall clock makes the configured delay easy to verify.
setlocal
set "SERVER=%~1"
if "%SERVER%"=="" set "SERVER=rtmp://127.0.0.1:1935/live/test"

set "FONT=C\:/Windows/Fonts/consola.ttf"
set "FILTER=drawtext=fontfile='%FONT%':text='%%{localtime\:%%X}':x=24:y=24:fontsize=64:fontcolor=white:box=1:boxcolor=black@0.6,drawtext=fontfile='%FONT%':text='InstantDelay test':x=24:y=104:fontsize=32:fontcolor=yellow"

ffmpeg -hide_banner -loglevel info -re ^
  -f lavfi -i "testsrc2=size=1280x720:rate=30" ^
  -f lavfi -i "sine=frequency=440:sample_rate=48000" ^
  -vf "%FILTER%" ^
  -c:v libx264 -preset veryfast -tune zerolatency -g 60 -keyint_min 60 -sc_threshold 0 -b:v 2500k -pix_fmt yuv420p ^
  -c:a aac -b:a 128k ^
  -f flv "%SERVER%"