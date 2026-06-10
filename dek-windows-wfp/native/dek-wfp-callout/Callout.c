#include "Callout.h"

UINT32 g_AleConnectCalloutId = 0;

VOID NTAPI PollenAleConnectClassifyFn(
    _In_ const FWPS_INCOMING_VALUES0* inFixedValues,
    _In_ const FWPS_INCOMING_METADATA_VALUES0* inMetaValues,
    _Inout_opt_ void* layerData,
    _In_opt_ const void* classifyContext,
    _In_ const FWPS_FILTER0* filter,
    _In_ UINT64 flowContext,
    _Inout_ FWPS_CLASSIFY_OUT0* classifyOut
) {
    UNREFERENCED_PARAMETER(layerData);
    UNREFERENCED_PARAMETER(classifyContext);
    UNREFERENCED_PARAMETER(filter);
    UNREFERENCED_PARAMETER(flowContext);

    if ((classifyOut->rights & FWPS_RIGHT_ACTION_WRITE) == 0) {
        return;
    }

    UINT8 protocol = 0;
    UINT16 remotePort = 0;

    for (UINT32 i = 0; i < inFixedValues->numValues; i++) {
        const FWPS_INCOMING_VALUE0* v = &inFixedValues->incomingValue[i];

        if (IsEqualGUID(&inFixedValues->layerId, &FWPS_LAYER_ALE_AUTH_CONNECT_V4)) {
            // At ALE_AUTH_CONNECT_V4, fields are indexed by FWPS_FIELD_ALE_AUTH_CONNECT_V4_*.
            // Use the generated field indexes, not GUIDs, in production.
        }
    }

    // Minimal demo behavior: permit everything.
    // Production: lookup process/app/user/remote tuple in a nonpaged cache.
    classifyOut->actionType = FWP_ACTION_PERMIT;
}

NTSTATUS NTAPI PollenAleConnectNotifyFn(
    _In_ FWPS_CALLOUT_NOTIFY_TYPE notifyType,
    _In_ const GUID* filterKey,
    _Inout_ FWPS_FILTER0* filter
) {
    UNREFERENCED_PARAMETER(notifyType);
    UNREFERENCED_PARAMETER(filterKey);
    UNREFERENCED_PARAMETER(filter);
    return STATUS_SUCCESS;
}

VOID NTAPI PollenAleConnectFlowDeleteFn(
    _In_ UINT16 layerId,
    _In_ UINT32 calloutId,
    _In_ UINT64 flowContext
) {
    UNREFERENCED_PARAMETER(layerId);
    UNREFERENCED_PARAMETER(calloutId);
    UNREFERENCED_PARAMETER(flowContext);
}

NTSTATUS PollenRegisterCallouts(_Inout_ void* deviceObject) {
    FWPS_CALLOUT0 callout = {0};
    callout.calloutKey = POLLEN_WFP_CALLOUT_ALE_CONNECT_V4;
    callout.classifyFn = PollenAleConnectClassifyFn;
    callout.notifyFn = PollenAleConnectNotifyFn;
    callout.flowDeleteFn = PollenAleConnectFlowDeleteFn;

    return FwpsCalloutRegister0(
        deviceObject,
        &callout,
        &g_AleConnectCalloutId
    );
}

VOID PollenUnregisterCallouts(void) {
    if (g_AleConnectCalloutId != 0) {
        FwpsCalloutUnregisterById0(g_AleConnectCalloutId);
        g_AleConnectCalloutId = 0;
    }
}
