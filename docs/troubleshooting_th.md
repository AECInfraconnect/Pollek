# คู่มือการแก้ไขปัญหาเบื้องต้น (Troubleshooting) - Pollek Local Enforcement Kit

## ปัญหาที่พบบ่อย

### 1. Mock-Cloud ไม่สามารถรันได้

**อาการ**: คำสั่ง `cargo run -p mock-cloud` แจ้งข้อผิดพลาด "Address already in use"
**วิธีแก้**: ตรวจสอบว่าพอร์ต 43891 และ 43892 ไม่ได้ถูกใช้งานโดยโปรแกรมอื่นอยู่ หากคุณใช้งานเครื่องพัฒนาร่วมกัน ให้ตรวจสอบว่ามี process ของ mock-cloud ค้างอยู่หรือไม่

### 2. การลงทะเบียน (Enrollment) ล้มเหลว

**อาการ**: คำสั่ง `Pollek-dekctl enroll` ค้าง หรือแสดงข้อผิดพลาดเกี่ยวกับการเชื่อมต่อ (Connection error)
**วิธีแก้**: ตรวจสอบให้แน่ใจว่า Mock-Cloud กำลังรันอยู่ และระบุ `--cloud-url` ไปยังพอร์ต HTTPS ของ Mock-Cloud ได้อย่างถูกต้อง (เช่น `https://127.0.0.1:43892`)

### 3. Local Enforcement Kit Core ไม่สามารถดึง (Sync) Bundle ได้

**อาการ**: ใน Log แสดง `bundle_sync_failed` และ Local Enforcement Kit เปลี่ยนไปใช้โหมด Fallback
**วิธีแก้**: ตรวจสอบว่าอุปกรณ์ได้รับการลงทะเบียนอย่างถูกต้องแล้ว และมีไฟล์ `bootstrap.json` ใน `~/.Pollek/Local Enforcement Kit/` หากคุณกำลังทดสอบสถานการณ์จำลอง (Chaos testing) ให้ตรวจสอบว่าคุณไม่ได้เผลอจำลองเหตุการณ์ Cloud Outage ไว้ใน Mock-Cloud

### 4. ไม่พบข้อมูล Telemetry ใน Dashboard

**อาการ**: หลังจากสั่งใช้งาน MCP Action แล้ว ไม่พบเหตุการณ์ใหม่ๆ ใน `/admin/dashboard`
**วิธีแก้**: ระบบจะทำการพักข้อมูล Telemetry ไว้ใน Buffer ชั่วคราว และจะส่งออกทุกๆ ช่วงเวลาที่กำหนด (ค่าเริ่มต้นคือ 5 วินาที) โปรดรอสักครู่เพื่อให้ข้อมูลถูกส่งออก หรือสั่ง Flush ด้วยตัวเอง ตรวจสอบด้วยว่า Local Enforcement Kit สามารถเชื่อมต่อเครือข่ายไปยัง Mock-Cloud ได้ตามปกติ

### 5. eBPF Guardrail ไม่ทำงาน (เฉพาะบน Linux)

**อาการ**: ระบบไม่ทำการบล็อกทราฟฟิก (Network egress) ตามที่ระบุไว้ในนโยบาย
**วิธีแก้**: ตรวจสอบให้แน่ใจว่า Local Enforcement Kit ถูกรันด้วยสิทธิ์ root (`CAP_BPF` และ `CAP_NET_ADMIN`) หรือใช้คำสั่ง `dmesg` หรือ `journalctl -u Pollek-Local Enforcement Kit` เพื่อตรวจสอบว่ามีข้อผิดพลาดเกี่ยวกับ BPF Verifier เกิดขึ้นหรือไม่

## ��õ�Ǩ�ͺ���� Preflight Doctor

�ҡ�ѹ \pollen-dek doctor\ ��������ҹ ����Ǩ�ͺ��Ǣ�ͷ������͹ �ѭ�ҷ�辺����:

- **����� WinDivert**: ����ͧ�ѹ��ǵԴ����ա���駴����Է��� Administrator.
- **���� 43889 �١��ҹ����**: ������ \
etstat -ano | findstr 43889\ ������ PID ��лԴ��������.
- **eBPF verifier error**: ������ Linux �ͧ�س�Ҩ����Թ� ��蹵���ش����ͧ�Ѻ��� 5.15.
