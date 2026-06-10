#pragma once
#include <ntddk.h>
#include <fwpsk.h>
#include <fwpmk.h>

// {6F667A2B-8BD0-4929-93CB-4EF8A0C4F200}
DEFINE_GUID(POLLEN_WFP_CALLOUT_ALE_CONNECT_V4,
    0x6f667a2b, 0x8bd0, 0x4929, 0x93, 0xcb, 0x4e, 0xf8, 0xa0, 0xc4, 0xf2, 0x00);

extern UINT32 g_AleConnectCalloutId;

NTSTATUS PollenRegisterCallouts(_Inout_ void* deviceObject);
VOID PollenUnregisterCallouts(void);
