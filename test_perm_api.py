#!/usr/bin/env python3
"""权限系统后端 API 层验证。"""
import json
import ssl
import urllib.request
import urllib.error

BASE = "https://127.0.0.1:8080"
CTX = ssl._create_unverified_context()
RESULTS = []


def call(method, path, body=None, token=None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(BASE + path, data=data, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Token", token)
    try:
        with urllib.request.urlopen(req, context=CTX, timeout=15) as r:
            return r.status, json.loads(r.read().decode())
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read().decode())
        except Exception:
            return e.code, {"msg": e.read().decode()[:200]}
    except Exception as e:
        return 0, {"msg": f"EXC: {e}"}


def check(name, cond, detail=""):
    RESULTS.append((name, cond))
    print(f"[{'PASS' if cond else 'FAIL'}] {name}" + (f"  -> {detail}" if detail else ""))


def rows_of(r):
    d = r.get("data")
    if isinstance(d, list):
        return d
    if isinstance(d, dict):
        return d.get("list", [])
    return []


def main():
    # 1. 登录
    TOKEN = None
    for pwd in ("admin123456", "123456", "admin", "admin123"):
        st, r = call("POST", "/api/admin/login", {"user": "admin", "password": pwd})
        if r.get("code") == 200:
            TOKEN = (r.get("data") or {}).get("token")
            print(f"  登录成功 pwd={pwd}")
            break
    if not TOKEN:
        check("登录 admin", False, "所有密码都不对")
        return
    check("登录 admin", True)

    # 2. add 部分权限
    st, r = call("POST", "/api/admin/admList/add",
                 {"user": "ptpartial", "password": "123456", "notes": "部分", "auth": ["cdk", "user"]}, TOKEN)
    check("add 部分权限", r.get("code") == 200, str(r)[:110])

    # 3. add 全不勾 []
    st, r = call("POST", "/api/admin/admList/add",
                 {"user": "ptnone", "password": "123456", "notes": "零权限", "auth": []}, TOKEN)
    check("add 空数组", r.get("code") == 200, str(r)[:110])

    # 4. add 通配符 all
    st, r = call("POST", "/api/admin/admList/add",
                 {"user": "ptall", "password": "123456", "notes": "通配", "auth": ["all"]}, TOKEN)
    check("add all 通配", r.get("code") == 200, str(r)[:110])

    # 5. add 非法组名
    st, r = call("POST", "/api/admin/admList/add",
                 {"user": "ptbad", "password": "123456", "notes": "非法", "auth": ["cdkss"]}, TOKEN)
    check("add 非法组名被拒", r.get("code") != 200 and "未知" in str(r.get("msg")), str(r)[:110])

    # 6. add 非字符串数组
    st, r = call("POST", "/api/admin/admList/add",
                 {"user": "ptnum", "password": "123456", "notes": "非串", "auth": [1, 2]}, TOKEN)
    check("add 非字符串数组被拒", r.get("code") != 200 and "字符串" in str(r.get("msg")), str(r)[:110])

    # 7. 查列表
    st, r = call("POST", "/api/admin/admList/list", {"pg": 1, "size": 50}, TOKEN)
    rows = rows_of(r)
    ids = {row["user"]: row for row in rows if str(row.get("user", "")).startswith("pt")}
    check("测试账号落库", len(ids) >= 3, f"{sorted(ids.keys())}")

    def auth_of(u):
        return ids.get(u, {}).get("auth")

    check("ptpartial auth=[cdk,user]", sorted(auth_of("ptpartial") or []) == ["cdk", "user"], str(auth_of("ptpartial")))
    check("ptnone auth=[]", auth_of("ptnone") == [], str(auth_of("ptnone")))
    check("ptall auth=[all]", (auth_of("ptall") or []) == ["all"], str(auth_of("ptall")))

    # 8. edit id=1 改权限 → 防死锁
    st, r = call("POST", "/api/admin/admList/edit", {"id": 1, "auth": ["cdk"]}, TOKEN)
    check("edit id=1 被拒", r.get("code") != 200 and "超级管理员" in str(r.get("msg")), str(r)[:140])

    # 9. edit pt partial → 改成 []（验证 [] 能真实写入）
    pid = ids.get("ptpartial", {}).get("id")
    st, r = call("POST", "/api/admin/admList/edit", {"id": pid, "auth": []}, TOKEN)
    check("edit -> [] 返回 200", r.get("code") == 200, str(r)[:110])
    st, r = call("POST", "/api/admin/admList/list", {"pg": 1, "size": 50}, TOKEN)
    new = [x["auth"] for x in rows_of(r) if x.get("id") == pid]
    check("[] 已落库", bool(new) and new[0] == [], str(new))

    # 10. edit pt partial → 改回 ["cdk"]
    st, r = call("POST", "/api/admin/admList/edit", {"id": pid, "auth": ["cdk"]}, TOKEN)
    check("edit -> [cdk] 返回 200", r.get("code") == 200, str(r)[:110])

    # 11. 受限账号访问受限接口 → 403
    st, r = call("POST", "/api/admin/login", {"user": "ptnone", "password": "123456"})
    t2 = (r.get("data") or {}).get("token") if r.get("code") == 200 else None
    if t2:
        st, r = call("POST", "/api/admin/admList/list", {"pg": 1, "size": 50}, t2)
        body = json.dumps(r, ensure_ascii=False)
        blocked = ("没有权限" in body) or ("403" in body) or r.get("code") == 403
        check("零权限 list 被拒", blocked, body[:120])
    else:
        check("零权限账号登录", False, str(r)[:120])

    # 12. 受限账号（ptpartial: cdk）访问 cdk 接口 → 应放行
    st, r = call("POST", "/api/admin/login", {"user": "ptpartial", "password": "123456"})
    t3 = (r.get("data") or {}).get("token") if r.get("code") == 200 else None
    if t3:
        st, r = call("GET", "/api/admin/cdkKami/list", {"pg": 1, "size": 10}, t3)
        body = json.dumps(r, ensure_ascii=False)
        ok = ("没有权限" not in body)
        check("部分权限 cdkKami 放行", ok, body[:120])
        st, r = call("POST", "/api/admin/admList/list", {"pg": 1, "size": 50}, t3)
        body = json.dumps(r, ensure_ascii=False)
        check("部分权限 admList 被拒", ("没有权限" in body) or r.get("code") == 403, body[:120])
    else:
        check("ptpartial 登录", False, str(r)[:120])

    # 13. 清理
    for u in ("ptpartial", "ptnone", "ptall"):
        _id = ids.get(u, {}).get("id")
        if _id:
            call("POST", "/api/admin/admList/del", {"id": _id}, TOKEN)
    st, r = call("POST", "/api/admin/admList/list", {"pg": 1, "size": 50}, TOKEN)
    left = [x["user"] for x in rows_of(r)]
    check("测试账号已清理", not any(u in left for u in ("ptpartial", "ptnone", "ptall")), str(left))

    print(f"\n===== {sum(1 for _, c in RESULTS if c)}/{len(RESULTS)} 通过 =====")


if __name__ == "__main__":
    main()
