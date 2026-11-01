@echo off
rem Uploads CarveCraft to https://github.com/simianguy78/CarveCraft
rem Double-click this file. GitHub may ask you to sign in the first time.
cd /d "%~dp0"
git remote get-url origin >nul 2>&1 || git remote add origin https://github.com/simianguy78/CarveCraft.git
git add -A
git diff --cached --quiet || git commit -m "Update CarveCraft"
rem Bring in anything that was changed on GitHub first, so the upload is not rejected.
git fetch origin
git rev-parse --verify --quiet origin/main >nul && git merge --no-edit --allow-unrelated-histories origin/main
if errorlevel 1 (
  echo.
  echo Changes on GitHub clash with changes here - nothing was uploaded.
  pause
  exit /b 1
)
git push -u origin main
echo.
if errorlevel 1 (
  echo Upload failed - see the message above.
) else (
  echo Done: https://github.com/simianguy78/CarveCraft
)
pause
