@echo off
rem Ouvre un terminal ou la commande `stlkm` est disponible.
set "PATH=%~dp0bin;%PATH%"
cd /d "%~dp0"
echo.
echo   Tape  stlkm  pour voir les commandes, ou par exemple :
echo.
echo     stlkm list        ce que contient le repertoire
echo     stlkm scan        les projets trouves sur le disque
echo     stlkm publish     envoyer le repertoire sur GitHub
echo.
cmd /k
