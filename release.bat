@echo off
chcp 65001 >nul

if "%1"=="" (
    echo 编译 Release 版本...
    cargo build --release
    upx -5 target/release/mcu-test.exe
    copy .\target\release\mcu-test.exe mcu-test.exe
    echo Release 版本编译完成！
) else (
    echo 编译 Debug 版本...
    cargo build 
    upx -5 target/debug/mcu-test.exe
    copy .\target\debug\mcu-test.exe mcu-test.exe
    echo Debug 版本编译完成！
)