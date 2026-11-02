@echo off
cd /d "%USERPROFILE%"
where wt.exe >nul 2>&1
if %errorlevel%==0 (
  start "" wt.exe -d "%USERPROFILE%" "%~dp0rst.exe"
) else (
  start "RST" "%~dp0rst.exe"
)
