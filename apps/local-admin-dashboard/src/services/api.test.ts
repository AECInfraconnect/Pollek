import { describe, expect, it } from "vitest";
import { isHtmlResponse, tenantBaseUrl } from "./api";

describe("ControlPlaneClient API response helpers", () => {
  it("detects dashboard HTML returned from an API route", () => {
    expect(
      isHtmlResponse(
        '<!doctype html><html lang="en"><head><title>Pollek</title></head>',
        "text/html; charset=utf-8",
      ),
    ).toBe(true);
  });

  it("does not treat JSON error payloads as HTML", () => {
    expect(
      isHtmlResponse(
        '{"error":"api route not found","message":"missing"}',
        "application/json",
      ),
    ).toBe(false);
  });

  it("builds tenant API bases from origins and existing bases", () => {
    expect(tenantBaseUrl("http://127.0.0.1:43891")).toBe(
      "http://127.0.0.1:43891/v1/tenants/local",
    );
    expect(tenantBaseUrl("http://127.0.0.1:43891/v1")).toBe(
      "http://127.0.0.1:43891/v1/tenants/local",
    );
    expect(tenantBaseUrl("")).toBe("/v1/tenants/local");
  });
});
