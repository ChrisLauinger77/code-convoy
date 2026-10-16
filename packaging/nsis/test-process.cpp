// MIT; disposable process fixture, never distributed in CodeConvoy packages.
#include <windows.h>
extern "C" void __cdecl start() {
    Sleep(INFINITE);
    ExitProcess(0);
}
