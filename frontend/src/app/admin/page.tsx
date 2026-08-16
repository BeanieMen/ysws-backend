"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";

type Role = "user" | "reviewer" | "admin";
interface CurrentUser { role: Role; }
interface Project { id: string; title: string; description?: string | null; shipped_at?: string | null; submission_status: string; }
interface User { id: string; email: string; first_name: string; last_name: string; role: Role; }

export default function AdminDashboard() {
  const router = useRouter();
  const [projects, setProjects] = useState<Project[]>([]);
  const [users, setUsers] = useState<User[]>([]);
  const [message, setMessage] = useState("");
  const [reviewStatus, setReviewStatus] = useState<Record<string, string>>({});
  const [reviewComment, setReviewComment] = useState<Record<string, string>>({});
  const [reviewingProjectId, setReviewingProjectId] = useState<string | null>(null);

  const fetchApi = useCallback(async <T,>(path: string, options: RequestInit = {}): Promise<T> => {
    const response = await fetch(path, { credentials: "include", ...options, headers: { "Content-Type": "application/json", ...(options.headers || {}) } });
    if (response.status === 401) { router.replace("/"); throw new Error("Sign in required"); }
    if (!response.ok) { const body: unknown = await response.json().catch(() => null); throw new Error(typeof body === "object" && body && "error" in body && typeof body.error === "string" ? body.error : "Request failed"); }
    return (response.status === 204 ? null : await response.json()) as T;
  }, [router]);

  const load = useCallback(async () => {
    const currentUser = await fetchApi<CurrentUser>("/api/v1/me");
    if (currentUser.role !== "admin") { router.replace("/dashboard"); return; }
    const [allProjects, allUsers] = await Promise.all([fetchApi<Project[]>("/api/v1/reviews/projects"), fetchApi<User[]>("/api/v1/admin/users")]);
    setProjects(allProjects); setUsers(allUsers);
  }, [fetchApi, router]);
  useEffect(() => {
    const loadInitialData = async () => {
      try {
        await load();
      } catch (error) {
        setMessage(error instanceof Error ? error.message : "Request failed");
      }
    };
    void loadInitialData();
  }, [load]);

  const updateRole = async (userId: string, role: Role) => {
    try { await fetchApi(`/api/v1/admin/users/${userId}/role`, { method: "PUT", body: JSON.stringify({ role }) }); await load(); }
    catch (error) { setMessage(error instanceof Error ? error.message : "Unable to update role"); }
  };

  const submitReview = async (projectId: string) => {
    setReviewingProjectId(projectId);
    setMessage("");
    try {
      await fetchApi(`/api/v1/projects/${projectId}/reviews`, {
        method: "POST",
        body: JSON.stringify({ status: reviewStatus[projectId] || "approved", comment: reviewComment[projectId] || null }),
      });
      setMessage("Review saved.");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Unable to save review");
    } finally {
      setReviewingProjectId(null);
    }
  };

  const shipped = projects.filter((project) => project.shipped_at).length;
  return (
    <main className="min-h-screen bg-slate-50 text-slate-900">
      <header className="max-w-6xl mx-auto px-6 h-20 flex items-center justify-between border-b border-slate-200">
        <Link href="/admin" className="font-mono text-xl font-extrabold">test-instance<span className="text-[#ec3750]">_</span></Link>
        <span className="text-sm font-bold text-[#ec3750]">Admin</span>
      </header>
      <section className="max-w-6xl mx-auto px-6 py-10">
        <p className="uppercase tracking-widest text-xs font-bold text-[#ec3750]">Admin dashboard</p>
        <h1 className="text-3xl font-extrabold mt-2">Projects and access</h1>
        <p className="text-sm text-slate-500 mt-2">{shipped} shipped · {projects.length - shipped} unshipped. Unshipped projects are visible only here.</p>
        {message && <p className="mt-4 text-sm font-semibold text-[#d62740]">{message}</p>}
        <div className="grid lg:grid-cols-2 gap-8 mt-8">
          <section><h2 className="font-extrabold text-xl mb-4">All projects</h2><div className="grid gap-3">
            {projects.map((project) => <article key={project.id} className="bg-white border border-slate-200 rounded-xl p-4"><div className="flex justify-between gap-3"><div><h3 className="font-bold">{project.title}</h3>{project.description && <p className="text-sm text-slate-500 mt-1">{project.description}</p>}</div><span className={`h-fit text-xs font-bold px-2 py-1 rounded-full ${project.shipped_at ? "bg-emerald-100 text-emerald-700" : "bg-amber-100 text-amber-700"}`}>{project.shipped_at ? "Shipped" : "Unshipped"}</span></div><div className="grid gap-2 mt-4"><select value={reviewStatus[project.id] || "approved"} onChange={(event) => setReviewStatus({ ...reviewStatus, [project.id]: event.target.value })} className="rounded-lg border border-slate-200 px-2 py-1 text-sm"><option value="approved">Approve</option><option value="changes_requested">Request changes</option><option value="rejected">Reject</option><option value="pending">Keep pending</option></select><input value={reviewComment[project.id] || ""} onChange={(event) => setReviewComment({ ...reviewComment, [project.id]: event.target.value })} maxLength={2000} placeholder="Optional review note" className="rounded-lg border border-slate-200 px-2 py-1 text-sm" /><button onClick={() => submitReview(project.id)} disabled={reviewingProjectId === project.id} className="rounded-lg bg-[#ec3750] px-3 py-2 text-sm font-bold text-white hover:bg-[#d62740] disabled:opacity-60">{reviewingProjectId === project.id ? "Saving…" : "Save review"}</button></div></article>)}
            {projects.length === 0 && <p className="text-sm text-slate-400">No projects yet.</p>}
          </div></section>
          <section><h2 className="font-extrabold text-xl mb-4">Users and roles</h2><div className="grid gap-3">
            {users.map((user) => <article key={user.id} className="bg-white border border-slate-200 rounded-xl p-4 flex items-center justify-between gap-3"><div><h3 className="font-bold">{user.first_name} {user.last_name}</h3><p className="text-sm text-slate-500">{user.email}</p></div><select value={user.role} onChange={(event) => updateRole(user.id, event.target.value as Role)} className="rounded-lg border border-slate-200 px-2 py-1 text-sm font-semibold"><option value="user">User</option><option value="reviewer">Reviewer</option><option value="admin">Admin</option></select></article>)}
          </div></section>
        </div>
      </section>
    </main>
  );
}
