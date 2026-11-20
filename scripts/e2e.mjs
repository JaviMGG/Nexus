const BASE = process.env.NEXUS_E2E_URL ?? "http://127.0.0.1:47899";
let token = null;
let passed = 0, failed = 0;

function check(name, cond, extra = "") {
  if (cond) { passed++; console.log(`  PASS  ${name}`); }
  else { failed++; console.log(`  FAIL  ${name} ${extra}`); }
}

async function cmd(command, payload = {}, useToken = true) {
  const res = await fetch(`${BASE}/api/command`, {
    method: "POST",
    headers: { "Content-Type": "application/json", ...(useToken ? { Authorization: `Bearer ${token}` } : {}) },
    body: JSON.stringify({ command, payload }),
  });
  const body = await res.json();
  return { status: res.status, ...body };
}

const session = await (await fetch(`${BASE}/api/session`)).json();
token = session.token;
check("session token", typeof token === "string" && token.length > 10);

// auth
const noAuth = await cmd("tasks.list", {}, false);
check("auth requerida (403 sin token)", noAuth.status === 403);

// tasks CRUD
const t = await cmd("tasks.create", { data: { title: "Preparar portfolio", due_date: "2026-11-27", priority: "high" } });
check("tasks.create", t.status === 200 && t.result.title === "Preparar portfolio", JSON.stringify(t));
const tid = t.result.id;
const tl = await cmd("tasks.list", { filter: {} });
check("tasks.list", tl.result.length === 1);
const tu = await cmd("tasks.update", { id: tid, data: { status: "done" } });
check("tasks.update done + completed_at", tu.result.status === "done" && !!tu.result.completed_at);

// captura sin IA
const cap = await cmd("capture.analyze", { content: "Tengo que preparar el portfolio antes del viernes" });
check("capture.analyze heurística sin IA", cap.result.kind === "task" && cap.result.ai_used === false && !!cap.result.task.due_date, JSON.stringify(cap.result));

// notas + FTS
await cmd("notes.create", { data: { title: "Rust async", content: "tokio y axum para servicios locales" } });
await cmd("notes.create", { data: { title: "Recetas", content: "tortilla de patatas" } });
const ns = await cmd("notes.search", { query: "tokio" });
check("notes FTS search", ns.result.length === 1 && ns.result[0].title === "Rust async");

// inbox
const ib = await cmd("inbox.create", { content: "llamar al banco" });
const ip = await cmd("inbox.process", { id: ib.result.id, kind: "task" });
check("inbox.process → tarea", ip.status === 200 && !!ip.result.created_id);

// proyectos + progreso
const p = await cmd("projects.create", { data: { name: "NEXUS", goal: "centro de control" } });
await cmd("tasks.create", { data: { title: "t1", project_id: p.result.id } });
const t2 = await cmd("tasks.create", { data: { title: "t2", project_id: p.result.id } });
await cmd("tasks.update", { id: t2.result.id, data: { status: "done" } });
const pd = await cmd("projects.get", { id: p.result.id });
check("projects progreso 50%", pd.result.progress.total === 2 && pd.result.progress.done === 1 && Math.abs(pd.result.progress.progress - 0.5) < 1e-9);
const plp = await cmd("projects.list_with_progress");
check("projects.list_with_progress", Array.isArray(plp.result) && plp.result[0].progress !== undefined);

// chat sin IA (modo degradado)
const send = await cmd("chat.send", { content: "hola NEXUS" });
check("chat.send sin IA → ai_available false", send.status === 200 && send.result.ai_available === false && send.result.run_id === null);
const msgs = await cmd("chat.messages", { conversation_id: send.result.conversation_id });
check("chat modo manual responde", msgs.result.some((m) => m.role === "assistant" && m.content.includes("no hay ningún proveedor")));

// agentes sin IA
const agents = await cmd("agents.list");
check("agents.list 6 agentes", agents.result.length === 6);
const run = await cmd("agents.start", { goal: "organiza mi semana" });
check("agents.start sin IA → ai_unavailable", run.status === 503 && run.error.code === "ai_unavailable", JSON.stringify(run));

// settings + paletas
await cmd("settings.update", { patch: { palette: "ocean", theme: "dark", user_name: "Javier" } });
const st = await cmd("settings.get");
check("settings paleta ocean persistida", st.result.app.palette === "ocean" && st.result.app.user_name === "Javier");
check("ai_status disconnected sin proveedor", st.result.ai_status.state === "disconnected");

// proveedores
const prov = await cmd("providers.save", { config: { id: "", name: "Qwen", kind: "openai_compatible", base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1", model: "qwen-max", api_key_set: false, temperature: 0.7, max_tokens: 4096, enabled: true, is_default: true, requests_per_minute: 30 } });
check("providers.save sin key", prov.status === 200 && prov.result.api_key_set === false);
const provKey = await cmd("providers.save", { config: { ...prov.result }, api_key: "sk-test-123" });
check("providers.save con key → api_key_set", provKey.result.api_key_set === true);
const st2 = await cmd("settings.get");
check("key nunca se expone", !JSON.stringify(st2.result).includes("sk-test-123"));
const test = await cmd("providers.test", { id: prov.result.id });
check("providers.test error claro (key falsa)", test.status >= 400 && /credenciales|401|inválid/i.test(test.error.message), JSON.stringify(test));
await cmd("providers.delete", { id: prov.result.id });

// undo
const del = await cmd("tasks.create", { data: { title: "para borrar" } });
await cmd("tasks.delete", { id: del.result.id });
const act = await cmd("activity.list", { limit: 50, entity_type: "task", entity_id: del.result.id });
const delEvent = act.result.find((e) => e.kind === "deleted");
const undo = await cmd("activity.undo", { id: delEvent.id });
check("activity.undo restaura", undo.status === 200);
const back = await cmd("tasks.get", { id: del.result.id });
check("tarea restaurada tras undo", back.result.task.title === "para borrar");

// backups
const bk = await cmd("backup.create");
check("backup.create", bk.status === 200 && bk.result.path.endsWith(".sqlite"));
const bl = await cmd("backup.list");
check("backup.list", bl.result.paths.length === 1);
const exp = await cmd("backup.export");
check("backup.export sin secretos", exp.result.tasks.length > 0 && !JSON.stringify(exp.result).includes("sk-test-123"));

// SSE en vivo
const ssePromise = (async () => {
  const res = await fetch(`${BASE}/api/events?token=${token}`);
  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buf = "";
  const deadline = Date.now() + 5000;
  while (Date.now() < deadline) {
    const { value, done } = await Promise.race([reader.read(), new Promise((r) => setTimeout(() => r({ done: true }), 4000))]);
    if (done) break;
    buf += decoder.decode(value, { stream: true });
    if (buf.includes("data_changed")) break;
  }
  reader.cancel().catch(() => {});
  return buf;
})();
await new Promise((r) => setTimeout(r, 500));
await cmd("tasks.create", { data: { title: "evento SSE" } });
const sseBuf = await ssePromise;
check("SSE data_changed en vivo", sseBuf.includes("data_changed"), sseBuf.slice(0, 120));

// comando desconocido
const unk = await cmd("no.existe");
check("comando desconocido → 404", unk.status === 404);

// dashboard
const dash = await cmd("dashboard.summary");
check("dashboard.summary", dash.status === 200 && dash.result.stats.open_tasks >= 1 && dash.result.user_name === "Javier");

console.log(`\nRESULTADO: ${passed} PASS, ${failed} FAIL`);
process.exit(failed ? 1 : 0);
