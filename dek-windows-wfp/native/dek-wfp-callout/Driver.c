#include <ntddk.h>
#include "Callout.h"

DRIVER_UNLOAD PollenDriverUnload;

PDEVICE_OBJECT g_DeviceObject = NULL;
UNICODE_STRING g_DeviceName = RTL_CONSTANT_STRING(L"\\Device\\PollenDekWfp");
UNICODE_STRING g_SymbolicLink = RTL_CONSTANT_STRING(L"\\DosDevices\\PollenDekWfp");

VOID PollenDriverUnload(_In_ PDRIVER_OBJECT DriverObject) {
    UNREFERENCED_PARAMETER(DriverObject);

    PollenUnregisterCallouts();

    IoDeleteSymbolicLink(&g_SymbolicLink);
    if (g_DeviceObject) {
        IoDeleteDevice(g_DeviceObject);
        g_DeviceObject = NULL;
    }
}

NTSTATUS DriverEntry(
    _In_ PDRIVER_OBJECT DriverObject,
    _In_ PUNICODE_STRING RegistryPath
) {
    UNREFERENCED_PARAMETER(RegistryPath);

    DriverObject->DriverUnload = PollenDriverUnload;

    NTSTATUS status = IoCreateDevice(
        DriverObject,
        0,
        &g_DeviceName,
        FILE_DEVICE_NETWORK,
        FILE_DEVICE_SECURE_OPEN,
        FALSE,
        &g_DeviceObject
    );
    if (!NT_SUCCESS(status)) {
        return status;
    }

    status = IoCreateSymbolicLink(&g_SymbolicLink, &g_DeviceName);
    if (!NT_SUCCESS(status)) {
        IoDeleteDevice(g_DeviceObject);
        g_DeviceObject = NULL;
        return status;
    }

    status = PollenRegisterCallouts(g_DeviceObject);
    if (!NT_SUCCESS(status)) {
        IoDeleteSymbolicLink(&g_SymbolicLink);
        IoDeleteDevice(g_DeviceObject);
        g_DeviceObject = NULL;
        return status;
    }

    return STATUS_SUCCESS;
}
