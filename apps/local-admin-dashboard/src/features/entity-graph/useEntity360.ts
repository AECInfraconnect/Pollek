import { useEffect, useState } from "react";
import { EntityGraphApi } from "../../services/entityGraphApi";
import type { Entity360Response } from "./types";

export function useEntity360(entityType?: string, entityId?: string) {
  const [data, setData] = useState<Entity360Response | null>(null);
  const [loading, setLoading] = useState(Boolean(entityType && entityId));
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    if (!entityType || !entityId) {
      setData(null);
      setLoading(false);
      return;
    }
    let mounted = true;
    setLoading(true);
    EntityGraphApi.getEntity360(entityType, entityId)
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
  }, [entityType, entityId]);

  return { data, loading, error };
}
