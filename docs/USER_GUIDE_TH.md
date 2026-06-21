# คู่มือการใช้งาน Pollen DEK

## ภาพรวม

Pollen DEK (Distributed Enforcement Kernel) คือเครื่องมือสำหรับรักษาความปลอดภัยระดับ endpoint และบังคับใช้นโยบาย (policy enforcement)

## ส่วนประกอบสำคัญ

- **Pollen DEK Core (`pollen-dek`)**: เซอร์วิสเบื้องหลังที่จัดการการยืนยันตัวตน ดาวน์โหลดนโยบาย และควบคุมการบังคับใช้
- **Pollen DEK CLI (`pollen-dekctl`)**: เครื่องมือ Command-line สำหรับลงทะเบียน จัดการ และตรวจสอบการทำงานของ DEK
- **Pollen MCP Proxy (`pollen-mcp-proxy`)**: พร็อกซีสำหรับการใช้งาน Model Context Protocol (MCP) ช่วยตรวจสอบสิทธิ์ก่อนส่งคำขอไปยังเครื่องมือต่างๆ
- **Mock Cloud (`pollen-mock-cloud`)**: ระบบจำลองการทำงานของ Pollen Cloud สำหรับการพัฒนาและทดสอบในช่วง Beta

## ฟีเจอร์ของ Local Admin Dashboard

Local Admin Dashboard (เข้าถึงได้ที่ `http://127.0.0.1:3000` เมื่อรัน Local Control Plane) มีฟีเจอร์สำหรับจัดการ DEK ภายในเครื่องของคุณ:

### 1. Simulator (จำลองการทำงาน)

ทดสอบนโยบายร่างหรือนโยบายที่ใช้งานอยู่โดยไม่ส่งผลกระทบต่อระบบจริง

- ไปที่เมนู **จำลองสถานการณ์ (Simulator)**
- ระบุข้อมูล subject, action, resource และ context ในรูปแบบ JSON
- ระบุผลลัพธ์ที่คาดหวัง (Expected Decision) เพื่อตรวจสอบความถูกต้อง
- คลิก **รันการจำลอง** เพื่อดูผลการประเมินจริง

### 2. Export Audit Logs (ดาวน์โหลดบันทึก)

ดาวน์โหลดบันทึกการตัดสินใจเพื่อนำไปวิเคราะห์ภายนอกหรือทำรายงาน Compliance

- ไปที่เมนู **บันทึกการตัดสินใจ (Decision Logs)**
- คลิก **Export CSV** หรือ **Export JSON** เพื่อดาวน์โหลดบันทึกจากระบบ

### 3. Connector Configuration (การเชื่อมต่อ PDP)

ตั้งค่าและทดสอบการเชื่อมต่อไปยัง External Policy Decision Points (PDPs) เช่น OPA, OpenFGA และ Cedar

- ไปที่เมนู **การตั้งค่า (Settings)**
- เพิ่มการเชื่อมต่อใหม่
- คลิก **Test Connection** เพื่อตรวจสอบว่าระบบสามารถเชื่อมต่อได้สำเร็จหรือไม่

## การตั้งค่า

ในช่วงทดสอบ Beta ไฟล์การตั้งค่าจะอยู่ที่ `~/.pollen/dek/` โดยค่าเริ่มต้น ซึ่งจะใช้ไฟล์ `bootstrap.json`

## บันทึกการทำงาน (Logs)

สามารถดู Logs ได้โดยใช้คำสั่ง `pollen-dekctl logs` หรือเปิดดูไฟล์ในโฟลเดอร์ `~/.pollen/dek/logs/`
