@echo off
setlocal enabledelayedexpansion

echo [32m正在检查环境...[0m

:: 检查字体文件
if not exist "assets\fonts\LXGWWenKaiGBScreen.ttf" (
    echo [33m警告: 字体文件不存在，尝试复制...[0m
    
    if not exist "assets\fonts" mkdir "assets\fonts"
    
    if exist "LXGWWenKaiGBScreen.ttf" (
        copy "LXGWWenKaiGBScreen.ttf" "assets\fonts\"
        echo [32m字体文件已复制到assets\fonts目录[0m
    ) else if exist "..\LXGWWenKaiGBScreen.ttf" (
        copy "..\LXGWWenKaiGBScreen.ttf" "assets\fonts\"
        echo [32m字体文件已复制到assets\fonts目录[0m
    ) else (
        echo [31m错误: 无法找到字体文件[0m
        echo [33m请下载字体文件并放置在assets\fonts目录[0m
        pause
        exit /b 1
    )
)

:: 检查是否需要编译
if not exist "target\release\rust_discord_bot.exe" (
    goto :compile
) else (
    if "%1"=="--rebuild" (
        goto :compile
    ) else (
        echo [32m使用现有的编译文件[0m
        goto :run
    )
)

:compile
echo [32m正在编译项目...[0m
cargo build --release
if %ERRORLEVEL% neq 0 (
    echo [31m编译失败！[0m
    pause
    exit /b 1
)
echo [32m编译完成[0m

:run
:: 创建必要的目录
if not exist "data\pic\temp" mkdir "data\pic\temp"
if not exist "data\sessions" mkdir "data\sessions"
if not exist "data\logs" mkdir "data\logs"

:: 设置日志环境变量
set RUST_LOG=info,rust_discord_bot=info

:: 创建日志文件名
for /f "tokens=2 delims==" %%a in ('wmic OS Get localdatetime /value') do set "dt=%%a"
set "YYYY=%dt:~0,4%"
set "MM=%dt:~4,2%"
set "DD=%dt:~6,2%"
set "HH=%dt:~8,2%"
set "Min=%dt:~10,2%"
set "Sec=%dt:~12,2%"
set "LOG_FILE=data\logs\win_bot_%YYYY%%MM%%DD%_%HH%%Min%%Sec%.log"

:: 运行程序
echo [32m启动Discord机器人...[0m
echo [32m日志将保存到: %LOG_FILE%[0m
target\release\rust_discord_bot.exe > %LOG_FILE% 2>&1

:: 检查退出状态
if %ERRORLEVEL% neq 0 (
    echo [31m程序异常退出，退出码: %ERRORLEVEL%[0m
    echo [33m检查日志获取更多信息[0m
    pause
    exit /b %ERRORLEVEL%
)

echo [32m程序正常退出[0m
pause 