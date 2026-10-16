// Copyright (c) 2026 CodeConvoy contributors
// SPDX-License-Identifier: MIT
// Original implementation using documented IShellLink/IPersistFile/IPropertyStore
// APIs. No ApplicationID plugin source or binary is used.
#include <windows.h>
#include <shobjidl.h>
#include <propkey.h>

// NSIS Unicode plugin ABI: stack nodes use the host's GlobalAlloc allocator.
struct Stack {
    Stack* next;
    wchar_t text[1];
};

static Stack* take(Stack** top) {
    if (!top || !*top) return nullptr;
    Stack* item = *top;
    *top = item->next;
    return item;
}

static bool terminated(const Stack* item, int capacity) {
    if (!item || capacity <= 0) return false;
    for (int i = 0; i < capacity; ++i) {
        if (item->text[i] == L'\0') return i > 0;
    }
    return false;
}

static HRESULT set_identity(const wchar_t* path, wchar_t* app_id) {
    IShellLinkW* link = nullptr;
    IPersistFile* file = nullptr;
    IPropertyStore* properties = nullptr;
    HRESULT initialized = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
    if (FAILED(initialized) && initialized != RPC_E_CHANGED_MODE) return initialized;
    HRESULT result = CoCreateInstance(CLSID_ShellLink, nullptr, CLSCTX_INPROC_SERVER,
                                     IID_IShellLinkW, reinterpret_cast<void**>(&link));
    if (SUCCEEDED(result)) result = link->QueryInterface(IID_PPV_ARGS(&file));
    if (SUCCEEDED(result)) result = file->Load(path, STGM_READWRITE);
    if (SUCCEEDED(result)) result = link->QueryInterface(IID_PPV_ARGS(&properties));
    if (SUCCEEDED(result)) {
        PROPVARIANT value = {};
        value.vt = VT_LPWSTR;
        // SetValue copies the value. This string is borrowed from the NSIS node;
        // PropVariantClear must not free it.
        value.pwszVal = app_id;
        result = properties->SetValue(PKEY_AppUserModel_ID, value);
    }
    if (SUCCEEDED(result)) result = properties->Commit();
    if (SUCCEEDED(result)) result = file->Save(path, TRUE);
    if (properties) properties->Release();
    if (file) file->Release();
    if (link) link->Release();
    if (SUCCEEDED(initialized)) CoUninitialize();
    return result;
}

extern "C" void __cdecl Set(HWND, int string_size, wchar_t*, Stack** top, void*) {
    Stack* path = take(top);
    Stack* id = take(top);
    bool valid = terminated(path, string_size) && terminated(id, string_size);
    HRESULT result = valid ? set_identity(path->text, id->text) : E_INVALIDARG;
    // Reuse a consumed host allocation: no additional allocation or CRT needed.
    Stack* status = path ? path : id;
    if (status && top && string_size >= 2) {
        status->text[0] = SUCCEEDED(result) ? L'0' : L'1';
        status->text[1] = L'\0';
        status->next = *top;
        *top = status;
    } else if (status) {
        GlobalFree(status);
    }
    if (id && id != status) GlobalFree(id);
}
