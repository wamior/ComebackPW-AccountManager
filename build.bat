@echo off
del target\debug\Login.exe
echo Build
cargo build
copy target\debug\Login.exe E:\Comeback\element\Login.exe