#include "dek_wfp_native.h"
#include <windows.h>
#include <fwpmu.h>
#include <fwpmtypes.h>
#include <winsock2.h>

#pragma comment(lib, "Fwpuclnt.lib")
#pragma comment(lib, "Ws2_32.lib")

static const GUID POLLEN_PROVIDER_KEY =
{ 0x6f667a2b, 0x8bd0, 0x4929, { 0x93, 0xcb, 0x4e, 0xf8, 0xa0, 0xc4, 0xf1, 0x00 } };
static const GUID POLLEN_SUBLAYER_KEY =
{ 0x6f667a2b, 0x8bd0, 0x4929, { 0x93, 0xcb, 0x4e, 0xf8, 0xa0, 0xc4, 0xf1, 0x01 } };

static uint32_t open_engine(HANDLE* engine) {
    FWPM_SESSION0 session = {};
    session.displayData.name = const_cast<wchar_t*>(L"Pollen DEK WFP Native Session");
    return FwpmEngineOpen0(NULL, RPC_C_AUTHN_WINNT, NULL, &session, engine);
}

uint32_t dek_wfp_init_provider(void) {
    HANDLE engine = NULL;
    DWORD status = open_engine(&engine);
    if (status != ERROR_SUCCESS) return status;

    FwpmTransactionBegin0(engine, 0);

    FWPM_PROVIDER0 provider = {};
    provider.providerKey = POLLEN_PROVIDER_KEY;
    provider.displayData.name = const_cast<wchar_t*>(L"Pollen DEK WFP Provider");
    provider.flags = FWPM_PROVIDER_FLAG_PERSISTENT;

    status = FwpmProviderAdd0(engine, &provider, NULL);
    if (status != ERROR_SUCCESS && status != FWP_E_ALREADY_EXISTS) goto abort;

    FWPM_SUBLAYER0 sublayer = {};
    sublayer.subLayerKey = POLLEN_SUBLAYER_KEY;
    sublayer.displayData.name = const_cast<wchar_t*>(L"Pollen DEK Enforcement Sublayer");
    sublayer.providerKey = const_cast<GUID*>(&POLLEN_PROVIDER_KEY);
    sublayer.weight = 0x7000;
    sublayer.flags = FWPM_SUBLAYER_FLAG_PERSISTENT;

    status = FwpmSubLayerAdd0(engine, &sublayer, NULL);
    if (status != ERROR_SUCCESS && status != FWP_E_ALREADY_EXISTS) goto abort;

    status = FwpmTransactionCommit0(engine);
    FwpmEngineClose0(engine);
    return status;

abort:
    FwpmTransactionAbort0(engine);
    FwpmEngineClose0(engine);
    return status;
}

uint32_t dek_wfp_add_rule(const DekWfpRule* rule) {
    if (!rule) return ERROR_INVALID_PARAMETER;

    HANDLE engine = NULL;
    DWORD status = open_engine(&engine);
    if (status != ERROR_SUCCESS) return status;

    FWPM_FILTER_CONDITION0 conditions[2] = {};
    UINT32 conditionCount = 0;

    conditions[conditionCount].fieldKey = FWPM_CONDITION_IP_PROTOCOL;
    conditions[conditionCount].matchType = FWP_MATCH_EQUAL;
    conditions[conditionCount].conditionValue.type = FWP_UINT8;
    conditions[conditionCount].conditionValue.uint8 = rule->protocol;
    conditionCount++;

    if (rule->remote_port != 0) {
        conditions[conditionCount].fieldKey = FWPM_CONDITION_IP_REMOTE_PORT;
        conditions[conditionCount].matchType = FWP_MATCH_EQUAL;
        conditions[conditionCount].conditionValue.type = FWP_UINT16;
        conditions[conditionCount].conditionValue.uint16 = rule->remote_port;
        conditionCount++;
    }

    FWPM_FILTER0 filter = {};
    filter.displayData.name = const_cast<wchar_t*>(L"Pollen DEK Compiled Rule");
    filter.providerKey = const_cast<GUID*>(&POLLEN_PROVIDER_KEY);
    filter.subLayerKey = POLLEN_SUBLAYER_KEY;
    filter.layerKey = rule->direction == DEK_WFP_DIRECTION_INBOUND
        ? FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4
        : FWPM_LAYER_ALE_AUTH_CONNECT_V4;
    filter.numFilterConditions = conditionCount;
    filter.filterCondition = conditions;
    filter.weight.type = FWP_UINT8;
    filter.weight.uint8 = rule->weight;
    filter.flags = FWPM_FILTER_FLAG_PERSISTENT;

    switch (rule->action) {
        case DEK_WFP_ACTION_ALLOW:
            filter.action.type = FWP_ACTION_PERMIT;
            break;
        case DEK_WFP_ACTION_BLOCK:
            filter.action.type = FWP_ACTION_BLOCK;
            break;
        default:
            FwpmEngineClose0(engine);
            return ERROR_INVALID_PARAMETER;
    }

    UINT64 filterId = 0;
    status = FwpmFilterAdd0(engine, &filter, NULL, &filterId);
    FwpmEngineClose0(engine);
    return status;
}

uint32_t dek_wfp_clear_pollen_filters(void) {
    // Production implementation should enumerate filters by provider/sublayer
    // and delete only Pollen-owned filters.
    return ERROR_CALL_NOT_IMPLEMENTED;
}
