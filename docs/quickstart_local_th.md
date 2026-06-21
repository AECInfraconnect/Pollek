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

สำหรับ Linux/macOS หรือ PowerShell 7+:

```bash
cargo build --workspace
cd apps/local-admin-dashboard && npm install && npm run build && cd -
```

สำหรับ Windows PowerShell (เวอร์ชันเก่า):

```powershell
cargo build --workspace
cd apps/local-admin-dashboard; npm install; npm run build; cd ../..
```

## 2. เริ่ม Local Control Plane

สำหรับ Linux/macOS หรือ bash/Zsh:

```bash
DEK_LCP_DATA=./pollen-local-data \
DEK_LCP_DB="sqlite://./pollen-local.db?mode=rwc" \
DEK_LCP_AUTH_DISABLE=1 \
  ./target/debug/local-control-plane
```

สำหรับ Windows PowerShell:

```powershell
$env:DEK_LCP_DATA="./pollen-local-data"
$env:DEK_LCP_DB="sqlite://./pollen-local.db?mode=rwc"
$env:DEK_LCP_AUTH_DISABLE="1"
.\target\debug\local-control-plane.exe
```

ตอนเริ่มจะ log public key ที่ใช้เซ็น bundle (`http://127.0.0.1:3000`)

## 3. ชี้ DEK ไปที่ Local Control Plane

> **สำหรับผู้ใช้ Windows PowerShell:** ให้เปิด **หน้าต่าง PowerShell ใหม่ (หรือแท็บใหม่)** สำหรับขั้นตอนนี้เป็นต้นไป โดยปล่อยหน้าต่างของข้อ 2 ให้ทำงานค้างไว้

สำหรับ Linux/macOS หรือ bash/Zsh:

```bash
# คัดลอก Trust key จากหน้าต่าง Log ของข้อ 2 (ส่วนที่อยู่ในวงเล็บ 'pub Base64EncodedKey==') มาใส่
# (สำหรับผู้ใช้ bash/Zsh สามารถใช้ curl ดึงมาได้ ถ้าปิด auth ไว้)
# curl -s http://127.0.0.1:3000/v1/tenants/local/devices/_/trusted-keys

./target/debug/dek-cli profile set local --url http://127.0.0.1:3000 --trusted-key "Base64EncodedKey=="
./target/debug/dek-cli profile show
```

สำหรับ Windows PowerShell:

```powershell
# คัดลอก Trust key จากหน้าต่าง Log ของข้อ 2 (ส่วนที่อยู่ในวงเล็บ 'pub Base64EncodedKey==') มาใส่
.\target\debug\dek-cli.exe profile set local --url http://127.0.0.1:3000 --trusted-key "Base64EncodedKey=="
.\target\debug\dek-cli.exe profile show
```

## 4. รัน DEK

*(หมายเหตุ: ใน Local Mode คำสั่ง `profile set local` ได้ทำการสร้างไฟล์ตั้งค่าไปแล้ว จึงไม่ต้องรัน `dek-cli enroll` ซ้ำ)*

สำหรับ Linux/macOS หรือ bash/Zsh:

```bash
./target/debug/dek-core &     # รัน dek-core เบื้องหลัง
./target/debug/dek-cli doctor
./target/debug/dek-cli status
```

สำหรับ Windows PowerShell:

```powershell
# dek-core จะทำงานค้างไว้คล้ายกับข้อ 2 หากต้องการรันเบื้องหลังให้ใช้คำสั่ง Start-Process 
# หรือสามารถเปิดหน้าต่างที่ 3 เพื่อรัน dek-core แยกต่างหากก็ได้
Start-Process .\target\debug\dek-core.exe -NoNewWindow

.\target\debug\dek-cli.exe doctor
.\target\debug\dek-cli.exe status
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

สำหรับ Linux/macOS หรือ bash/Zsh:

```bash
./target/debug/dek-cli profile set cloud --url https://cloud.<your-cloud-domain> --tenant-id your-tenant
./target/debug/dek-cli enroll --cloud-url https://cloud.<your-cloud-domain>
```

สำหรับ Windows PowerShell:

```powershell
.\target\debug\dek-cli.exe profile set cloud --url https://cloud.<your-cloud-domain> --tenant-id your-tenant
.\target\debug\dek-cli.exe enroll --cloud-url https://cloud.<your-cloud-domain>
```

## Guardrails (เปิดตลอด)

- DEK ไม่ author/compile policy ที่เครื่อง — ทำที่ control plane
- bundle ต้องเซ็นเสมอ; verify ไม่ผ่าน = reject (fail-closed)
- control plane ติดต่อไม่ได้ → ใช้ last-known-good; เกิน `max_bundle_age` → default deny

## แก้ปัญหา

- **เข้าหน้า Dashboard แล้วขึ้น HTTP 404:** โปรแกรม `local-control-plane` หาไฟล์หน้าเว็บไม่เจอ ให้ปิดมันก่อน (`Ctrl+C`) จากนั้นเซ็ตตัวแปร `$env:DEK_DASHBOARD_DIR=".\apps\local-admin-dashboard\dist"` (สำหรับ Windows) หรือ `export DEK_DASHBOARD_DIR="./apps/local-admin-dashboard/dist"` (สำหรับ Linux/macOS) แล้วค่อยรันขึ้นมาใหม่
- **เจอ Error `bootstrap already exists`:** มักเกิดจากการรัน `dek-cli enroll` ซ้ำ หรือมีไฟล์ตั้งค่าเก่าค้างอยู่ วิธีแก้คือ ปิด `dek-core`, ลบโฟลเดอร์ตั้งค่าทิ้ง (`C:\ProgramData\PollenDEK` ใน Windows) แล้วเริ่มทำข้อ 3 ใหม่อีกครั้ง
- **`dek-cli doctor`** ช่วยบอกปัญหา cert/connectivity/permission + วิธีแก้
- **ไม่มี decision ใน log?** เช็คว่า `dek-core` รันอยู่ + `dek-cli status` แสดง bundle ที่ sync แล้ว
- **bundle ถูก reject?** trust key ที่ pin ไม่ตรงกับ Local CP — ทำ step 3 ใหม่ด้วย `public_b64` ปัจจุบัน
