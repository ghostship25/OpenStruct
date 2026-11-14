@echo off
REM Build OpenStruct MSI installer
REM Requires: WiX Toolset v3.11+ (candle.exe, light.exe)

echo Building OpenStruct release...
cargo build --release -p openstruct-gui
if %ERRORLEVEL% NEQ 0 (
    echo Build failed!
    exit /b 1
)

echo Compiling WiX source...
candle.exe -out openstruct.wixobj openstruct.wxs
if %ERRORLEVEL% NEQ 0 (
    echo WiX compile failed!
    exit /b 1
)

echo Linking MSI...
light.exe -out OpenStruct-0.1.0.msi openstruct.wixobj
if %ERRORLEVEL% NEQ 0 (
    echo WiX link failed!
    exit /b 1
)

echo Installer created: OpenStruct-0.1.0.msi
