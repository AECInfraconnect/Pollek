import { useEffect, useState } from "react";
import { EntityGraphApi } from "../../services/entityGraphApi";
import type {
  EntityGraphQuery,
  EntityGraphResponse,
} from "./types";

export function useEntityGraph(params?: EntityGraphQuery) {
  const [data, setData] = useState<EntityGraphResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    let mounted = true;
    setLoading(true);
    EntityGraphApi.getGraph(params)
      .then((response) => {
        if (mounted) {
          setData(response);
          setError(null);
        }
      })
      .catch((err) => {
        if (mounted) setError(err instanceof Error ? err : new Error(String(err)));
      })
      .finally(() => {
        if (mounted) setLoading(false);
      });
    return () => {
      mounted = false;
    };
  }, [params?.types, params?.status, params?.q, params?.limit]);

  return { data, loading, error };
}
