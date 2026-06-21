# Pollen DEK — คู่มือเริ่มต้นแบบ Local Mode

รัน **ทั้ง stack ของ Pollen บนเครื่องเดียว** — ไม่ต้องมี Pollen Cloud
**Local Control Plane** ทำหน้าที่เป็น control plane แบบ single-user (`tenant_id=local`)
แทน Cloud: author policy, publish signed bundle แล้ว DEK เอาไป enforce พร้อมส่ง
decision log กลับมา ทั้งหมดบน localhost

> ใช้ schema / API contract / bundle format / telemetry envelope เดียวกับ Cloud
> เปลี่ยนไป Cloud ทีหลังแก้แค่ endpoint + trust store — โค้ด enforcement ของ DEK ไม่เปลี่ยน

## สิ่งที่ต้องมี
- Rust toolchain (stable) + Node 20+ (สำหรับ dashboard)
- Linux/macOS/Windows (network guardrail บังคับระดับ kernel เฉพาะ Linux; Win/macOS = redirect-advisory ใน beta)

## 1. Build
```bash
cargo build --workspace
cd apps/local-admin-dashboard && npm install && npm run build && cd -
```

## 2. เริ่ม Local Control Plane
```bash
DEK_LCP_DATA=./pollen-local-data \
DEK_LCP_DB="sqlite://./pollen-local.db?mode=rwc" \
  ./target/debug/local-control-plane
```
ตอนเริ่มจะ log public key ที่ใช้เซ็น bundle (`http://127.0.0.1:3000`)

## 3. ชี้ DEK ไปที่ Local Control Plane
```bash
curl -s http://127.0.0.1:3000/v1/tenants/local/devices/_/trusted-keys
dek-cli profile set local --url http://127.0.0.1:3000 --trusted-key "Base64EncodedKey=="
dek-cli profile show
```

## 4. Enroll + รัน DEK
```bash
dek-cli enroll --cloud-url http://127.0.0.1:3000
./target/debug/dek-core &     # PEP :43890
dek-cli doctor
dek-cli status
```

## 5. Author → Publish policy
ทำผ่าน dashboard (หน้า **Policy Enforcer**) หรือ API:
```bash
curl -X POST http://127.0.0.1:3000/v1/tenants/local/policies \
  -H 'content-type: application/json' \
  -d '{"meta":{"schema_version":"1.0","tenant_id":"local","workspace_id":"default",
       "environment_id":"local","created_at":"2026-06-10T00:00:00Z",
       "updated_at":"2026-06-10T00:00:00Z","created_by":"local-admin",
       "updated_by":"local-admin","source":"manual","status":"draft","tags":[]},
       "policy_id":"pol-allow-echo","name":"allow echo","policy_type":"cedar",
       "targets":{"agent_ids":[],"tool_ids":[],"resource_ids":[],"entity_ids":[],"route_ids":[]},
       "source":{"kind":"raw_text","language":"cedar","text":"permit(principal, action, resource);"},
       "compile_options":{"fail_on_warnings":true}}'

curl -X POST http://127.0.0.1:3000/v1/tenants/local/policies/pol-allow-echo/publish
```
DEK จะ sync bundle ใหม่ใน sync รอบถัดไป → verify ลายเซ็นกับ local key → hot-reload

## 6. Enforce + ดู decision log
```bash
curl -s -X POST http://127.0.0.1:43890/v1/authorize \
  -H 'content-type: application/json' \
  -d '{"mcp":{"method":"tools/call","params":{"name":"safe.echo"}},
       "principal":"me","tenant_id":"local","risk_tier":"low"}'

curl -s http://127.0.0.1:3000/v1/tenants/local/telemetry/decision-logs
```
ดูใน dashboard หน้า **Audit & Decision Logs** ได้เช่นกัน

## เกิดอะไรขึ้น
1. Local Control Plane **เซ็น** bundle ด้วย key ของตัวเอง
2. DEK **verify** เหมือน Cloud bundle เป๊ะ — fail-closed ถ้าลายเซ็นไม่ตรง
3. decision ส่งกลับด้วย **telemetry envelope เดียวกับ Cloud**

DEK จึงไม่รู้ว่ากำลังคุยกับ Local หรือ Cloud

## เปลี่ยนไป Pollen Cloud (ภายหลัง)
```bash
dek-cli profile set cloud --url https://cloud.pollen.ai --tenant-id your-tenant
dek-cli enroll --cloud-url https://cloud.pollen.ai
```

## Guardrails (เปิดตลอด)
- DEK ไม่ author/compile policy ที่เครื่อง — ทำที่ control plane
- bundle ต้องเซ็นเสมอ; verify ไม่ผ่าน = reject (fail-closed)
- control plane ติดต่อไม่ได้ → ใช้ last-known-good; เกิน `max_bundle_age` → default deny

## แก้ปัญหา
- `dek-cli doctor` บอกปัญหา cert/connectivity/permission + วิธีแก้
- ไม่มี decision ใน log? เช็คว่า `dek-core` รันอยู่ + `dek-cli status` แสดง bundle ที่ sync แล้ว
- bundle ถูก reject? trust key ที่ pin ไม่ตรงกับ Local CP — ทำ step 3 ใหม่ด้วย `public_b64` ปัจจุบัน
