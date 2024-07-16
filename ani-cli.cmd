@ECHO OFF
IF EXIST "%GIT_INSTALL_ROOT%\bin\bash.exe" SET ani-windows=%GIT_INSTALL_ROOT%\bin\bash.exe && GOTO :next
IF EXIST "%ProgramFiles%\Git\bin\bash.exe" SET ani-windows=%ProgramFiles%\Git\bin\bash.exe && GOTO :next
IF EXIST "%CMDER_ROOT%\vendor\git-for-windows\bin\bash.exe" SET ani-windows=%CMDER_ROOT%\vendor\git-for-windows\bin\bash.exe && GOTO :next
IF EXIST "%~dp0PortableGit" SET ani=%~dp0PortableGit\bin\bash.exe && GOTO :next
aria2c --allow-overwrite "https://github.com/git-for-windows/git/releases/download/v2.45.2.windows.1/PortableGit-2.45.2-64-bit.7z.exe" --dir="%~dp0\" -o "PortableGit.exe"
"%~dp0\PortableGit.exe" -y
del "%~dp0\PortableGit.exe"
SET ani-windows=%~dp0PortableGit\bin\bash.exe
:next
"%ani-windows%" %~dp0ani-cli %*
