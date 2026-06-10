#pragma once
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define DEK_WFP_DIRECTION_OUTBOUND 1
#define DEK_WFP_DIRECTION_INBOUND  2

#define DEK_WFP_ACTION_ALLOW 1
#define DEK_WFP_ACTION_BLOCK 2
#define DEK_WFP_ACTION_CALLOUT 3

typedef struct DekWfpRule {
    uint32_t direction;
    uint32_t action;
    uint8_t  protocol;
    uint16_t remote_port;
    uint32_t remote_ipv4_be;
    uint8_t  weight;
} DekWfpRule;

uint32_t dek_wfp_init_provider(void);
uint32_t dek_wfp_add_rule(const DekWfpRule* rule);
uint32_t dek_wfp_clear_pollen_filters(void);

#ifdef __cplusplus
}
#endif
