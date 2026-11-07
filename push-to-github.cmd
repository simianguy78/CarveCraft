@echo off
rem Uploads CarveCraft to https://github.com/simianguy78/CarveCraft
rem Double-click this file. GitHub may ask you to sign in the first time.
cd /d "%~dp0"
git remote get-url origin >nul 2>&1 || git remote add origin https://github.com/simianguy78/CarveCraft.git
git add -A
git diff --cached --quiet || git commit -m "Update CarveCraft"
git fetch origin

rem One time only: GitHub still has the old history with the old email address. Replace it
rem with the rewritten one (every save showing simianguy@gmail.com). --force-with-lease only
rem replaces it if GitHub still holds exactly that old version, so nothing newer is lost.
set OLD=6c478d625f77ed4309f5c7b17f0df870d31dff8e
for /f %%h in ('git rev-parse origin/main 2^>nul') do set REMOTE=%%h
if "%REMOTE%"=="%OLD%" (
  echo Replacing GitHub's history once so every save shows simianguy@gmail.com ...
  git push --force-with-lease=main:%OLD% -u origin main
  goto result
)

rem Bring in anything that was changed on GitHub first, so the upload is not rejected.
git rev-parse --verify --quiet origin/main >nul && git merge --no-edit --allow-unrelated-histories origin/main
if errorlevel 1 (
  echo.
  echo Changes on GitHub clash with changes here - nothing was uploaded.
  pause
  exit /b 1
)
git push -u origin main

:result
echo.
if errorlevel 1 (
  echo Upload failed - see the message above.
  pause
  exit /b 1
)
rem After a good upload, refresh our copy of GitHub's state and clear out anything no
rem longer used (such as the old-email history this replaced), so no trace stays here.
git fetch origin --prune -q
git reflog expire --expire=now --all
git gc --prune=now -q
echo Done: https://github.com/simianguy78/CarveCraft
pause
