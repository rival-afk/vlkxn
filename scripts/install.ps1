<#
.SYNOPSIS
    Vlkxn Windows Installer — устанавливает Vlkxn P2P VPN
.DESCRIPTION
    - Копирует файлы в %ProgramFiles%\Vlkxn
    - Устанавливает wintun драйвер (требует админских прав)
    - Создаёт ярлыки в меню Пуск и на рабочем столе
    - Добавляет в PATH
#>

param(
    [switch]$Portable,
    [string]$InstallDir = "$env:ProgramFiles\Vlkxn"
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

function Write-Banner {
    Write-Host "╔══════════════════════════════════╗" -ForegroundColor DarkRed
    Write-Host "║   🌋 Vlkxn Windows Installer     ║" -ForegroundColor DarkRed
    Write-Host "╚══════════════════════════════════╝" -ForegroundColor DarkRed
}

function Test-Admin {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Install-Wintun {
    Write-Host ">> Установка wintun драйвера..." -ForegroundColor Yellow
    $wintunUrl = "https://www.wintun.net/builds/wintun-0.14.1.zip"
    $zipPath = "$env:TEMP\wintun.zip"
    $extractPath = "$env:TEMP\wintun"

    try {
        Invoke-WebRequest -Uri $wintunUrl -OutFile $zipPath -UseBasicParsing
        Expand-Archive -Path $zipPath -DestinationPath $extractPath -Force

        $arch = if ([Environment]::Is64BitOperatingSystem) { "amd64" } else { "x86" }
        $dllPath = "$extractPath\wintun\bin\$arch\wintun.dll"
        
        if (Test-Path $dllPath) {
            Copy-Item $dllPath "$InstallDir\wintun.dll" -Force
            Write-Host "   [+] wintun.dll установлен" -ForegroundColor Green
        }
    } catch {
        Write-Warning "Не удалось загрузить wintun: $_"
        Write-Host "   [!] wintun.dll нужно скачать вручную с https://www.wintun.net" -ForegroundColor Yellow
    } finally {
        Remove-Item $zipPath -ErrorAction SilentlyContinue
        Remove-Item $extractPath -Recurse -ErrorAction SilentlyContinue
    }
}

function Install-Vlkxn {
    Write-Host ">> Установка Vlkxn в $InstallDir ..." -ForegroundColor Yellow

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null

    # Копируем бинарники
    $binaries = @("vlkxn-cli.exe", "vlkxn-gui.exe")
    foreach ($bin in $binaries) {
        $src = Join-Path $ScriptDir $bin
        if (Test-Path $src) {
            Copy-Item $src $InstallDir -Force
            Write-Host "   [+] $bin" -ForegroundColor Green
        }
    }

    # Копируем README и LICENSE
    foreach ($file in @("README.md", "LICENSE")) {
        $src = Join-Path $ScriptDir $file
        if (Test-Path $src) {
            Copy-Item $src $InstallDir -Force
        }
    }

    Install-Wintun
}

function Create-Shortcuts {
    Write-Host ">> Создание ярлыков..." -ForegroundColor Yellow

    $wshell = New-Object -ComObject WScript.Shell

    # Меню Пуск
    $startMenu = "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\Vlkxn"
    New-Item -ItemType Directory -Path $startMenu -Force | Out-Null

    $shortcut = $wshell.CreateShortcut("$startMenu\Vlkxn.lnk")
    $shortcut.TargetPath = "$InstallDir\vlkxn-gui.exe"
    $shortcut.WorkingDirectory = $InstallDir
    $shortcut.Description = "Vlkxn — P2P VPN for Gaming"
    $shortcut.Save()
    Write-Host "   [+] Ярлык в меню Пуск" -ForegroundColor Green

    # Рабочий стол
    $desktop = [Environment]::GetFolderPath("Desktop")
    $shortcut = $wshell.CreateShortcut("$desktop\Vlkxn.lnk")
    $shortcut.TargetPath = "$InstallDir\vlkxn-gui.exe"
    $shortcut.WorkingDirectory = $InstallDir
    $shortcut.Description = "Vlkxn — P2P VPN for Gaming"
    $shortcut.Save()
    Write-Host "   [+] Ярлык на рабочем столе" -ForegroundColor Green

    # CLI в PATH
    $currentPath = [Environment]::GetEnvironmentVariable("Path", "Machine")
    if ($currentPath -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$currentPath;$InstallDir", "Machine")
        Write-Host "   [+] Vlkxn добавлен в PATH" -ForegroundColor Green
    }
}

function Show-Completion {
    Write-Host
    Write-Host "╔══════════════════════════════════╗" -ForegroundColor Green
    Write-Host "║   ✅ Vlkxn успешно установлен!   ║" -ForegroundColor Green
    Write-Host "╚══════════════════════════════════╝" -ForegroundColor Green
    Write-Host
    Write-Host "Запустите Vlkxn через меню Пуск или командой: vlkxn-cli --help" -ForegroundColor Cyan
    Write-Host
}

function Install-Portable {
    $portableDir = "$PWD\vlkxn-portable"
    Write-Host ">> Создание portable сборки в $portableDir ..." -ForegroundColor Yellow
    New-Item -ItemType Directory -Path $portableDir -Force | Out-Null

    foreach ($bin in @("vlkxn-cli.exe", "vlkxn-gui.exe")) {
        $src = Join-Path $ScriptDir $bin
        if (Test-Path $src) {
            Copy-Item $src $portableDir -Force
        }
    }
    foreach ($file in @("README.md", "LICENSE")) {
        $src = Join-Path $ScriptDir $file
        if (Test-Path $src) {
            Copy-Item $src $portableDir -Force
        }
    }

    Write-Host "   [+] Portable сборка готова: $portableDir" -ForegroundColor Green
}

# Main
Write-Banner

if ($Portable) {
    Install-Portable
    return
}

if (-not (Test-Admin)) {
    Write-Host "⚠️  Требуются права администратора для установки драйвера wintun!" -ForegroundColor Red
    Write-Host "   Запустите PowerShell от имени администратора и повторите." -ForegroundColor Yellow
    Write-Host
    Write-Host "   Или используйте флаг -Portable для portable версии без установки." -ForegroundColor Cyan
    exit 1
}

Install-Vlkxn
Create-Shortcuts
Show-Completion
