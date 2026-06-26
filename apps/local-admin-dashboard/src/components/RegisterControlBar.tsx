import { useState } from "react";

interface RegisterControlBarProps {
  agentId: string;
  tenantId: string;
  onSuccess?: () => void;
}

export const RegisterControlBar: React.FC<RegisterControlBarProps> = ({
  agentId,
  tenantId,
  onSuccess,
}) => {
  const [level, setLevel] = useState("Observe");
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<string | null>(null);

  const handleRegister = async () => {
    setLoading(true);
    setResult(null);
    try {
      const resp = await fetch(
        `/v1/tenants/${tenantId}/agents/${agentId}/register`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Authorization: `Bearer ${localStorage.getItem("pollek_token") || ""}`,
          },
          body: JSON.stringify({ level }),
        },
      );
      if (resp.ok) {
        setResult("Success!");
        if (onSuccess) onSuccess();
      } else {
        setResult("Error registering");
      }
    } catch (e) {
      setResult("Network error");
    }
    setLoading(false);
  };

  return (
    <div className="flex items-center gap-2">
      <select
        value={level}
        onChange={(e: React.ChangeEvent<HTMLSelectElement>) =>
          setLevel(e.target.value)
        }
        className="h-8 rounded-md border border-input bg-background text-foreground px-3 py-1 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring [&>option]:bg-background [&>option]:text-foreground"
      >
        <option value="Observe" className="bg-background text-foreground">
          Observe
        </option>
        <option value="Guard" className="bg-background text-foreground">
          Guard
        </option>
        <option value="Enforce" className="bg-background text-foreground">
          Enforce
        </option>
        <option value="Block" className="bg-background text-foreground">
          Block
        </option>
      </select>
      <button
        onClick={handleRegister}
        disabled={loading}
        className="inline-flex items-center justify-center whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 bg-primary text-primary-foreground shadow hover:bg-primary/90 h-8 px-3"
      >
        {loading ? "..." : "Deploy"}
      </button>
      {result && (
        <span
          className={`text-xs font-medium ${result === "Success!" ? "text-emerald-500" : "text-destructive"}`}
        >
          {result}
        </span>
      )}
    </div>
  );
};
