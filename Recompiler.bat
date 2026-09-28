@echo off
rem A lancer apres une modification du code : recompile et remplace les
rem programmes du dossier bin.
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
cd /d "%~dp0"
echo Compilation en cours, ca peut prendre une minute...
cargo build --release || goto :rate
copy /Y "target\release\stlkm-hub.exe" "bin\stlkm-hub.exe" >nul
copy /Y "target\release\stlkm.exe" "bin\stlkm.exe" >nul
echo.
echo Termine. Les programmes du dossier bin sont a jour.
pause
exit /b 0

:rate
echo.
echo La compilation a echoue : le message d'erreur est au-dessus.
pause
exit /b 1
