import { defaultClient } from "./api";
import type {
  ActivityTimelineQuery,
  ActivityTimelineResponse,
  Entity360Response,
  EntityGraphQuery,
  EntityGraphResponse,
} from "../features/entity-graph/types";

function queryString(params?: object) {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(params ?? {})) {
    if (value !== undefined && value !== null && value !== "") {
      query.set(key, String(value));
    }
  }
  const suffix = query.toString();
  return suffix ? `?${suffix}` : "";
}

export const EntityGraphApi = {
  getGraph(params?: EntityGraphQuery): Promise<EntityGraphResponse> {
    return defaultClient.fetchApi(`/entity-graph${queryString(params)}`);
  },

  getEntity360(
    entityType: string,
    entityId: string,
  ): Promise<Entity360Response> {
    return defaultClient.fetchApi(
      `/entity-graph/node${queryString({
        entity_type: entityType,
        entity_id: entityId,
      })}`,
    );
  },

  getPolicyImpact(policyId: string): Promise<Entity360Response> {
    return defaultClient.fetchApi(
      `/entity-graph/policy-impact/${encodeURIComponent(policyId)}`,
    );
  },

  getActivity(
    params?: ActivityTimelineQuery,
  ): Promise<ActivityTimelineResponse> {
    return defaultClient.fetchApi(`/activity-timeline${queryString(params)}`);
  },
};
