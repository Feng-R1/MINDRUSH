@echo off
setlocal
cd /d "%~dp0"
where node >nul 2>nul
if errorlevel 1 (
  echo Please install Node.js 20.19 or newer, or Node.js 22.12 or newer.
  pause
  exit /b 1
)
node -e "const [major,minor]=process.versions.node.split('.').map(Number);if(!(major===20&&minor>=19||major===22&&minor>=12||major>22))process.exit(1)"
if errorlevel 1 (
  echo MindRush requires Node.js 20.19 or newer, or Node.js 22.12 or newer.
  echo Download a supported release from https://nodejs.org/.
  pause
  exit /b 1
)
where cargo >nul 2>nul
if errorlevel 1 (
  echo Please install Rust stable and Microsoft C++ Build Tools.
  echo For use without developer tools, download the installer from GitHub Releases.
  pause
  exit /b 1
)
if not exist "frontend\node_modules\.package-lock.json" (
  call npm ci --prefix frontend
  if errorlevel 1 (
    pause
    exit /b 1
  )
)
call npm run desktop:dev
if errorlevel 1 pause
