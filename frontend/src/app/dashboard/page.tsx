"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";

interface Project {
  id: string;
  title: string;
  description?: string;
  linked_project_names: string[];
  total_seconds: number;
}

interface DashboardData {
  projects: Project[];
  total_seconds: number;
}

interface HackatimeProject {
  name: string;
  total_duration?: number;
}

interface CurrentUser {
  first_name: string;
  hackatime_connected: boolean;
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : "Request failed";
}

export default function Dashboard() {
  const router = useRouter();
  const [userName, setUserName] = useState("");
  const [hackatimeConnected, setHackatimeConnected] = useState(true);
  const [dashboard, setDashboard] = useState<DashboardData>({ projects: [], total_seconds: 0 });
  const [availableProjects, setAvailableProjects] = useState<HackatimeProject[]>([]);
  const [loading, setLoading] = useState(true);

  // New project form state
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [createError, setCreateError] = useState("");

  // Modal linking state
  const [selectedProject, setSelectedProject] = useState<Project | null>(null);
  const [selectedNames, setSelectedNames] = useState<string[]>([]);
  const [linkError, setLinkError] = useState("");

  const fetchApi = useCallback(async <T,>(path: string, options: RequestInit = {}): Promise<T> => {
    const res = await fetch(path, {
      credentials: "include",
      ...options,
      headers: {
        "Content-Type": "application/json",
        ...(options.headers || {}),
      },
    });

    if (res.status === 401) {
      router.replace("/");
      throw new Error("Sign in required");
    }
    if (!res.ok) {
      const body: unknown = await res.json().catch(() => null);
      const message = typeof body === "object" && body !== null && "error" in body && typeof body.error === "string"
        ? body.error
        : "Request failed";
      throw new Error(message);
    }
    return (res.status === 204 ? null : await res.json()) as T;
  }, [router]);

  const loadData = useCallback(async () => {
    try {
      const [dash, ht] = await Promise.all([
        fetchApi<DashboardData>("/api/v1/projects"),
        fetchApi<{ projects: HackatimeProject[] }>("/api/v1/hackatime/projects").catch(() => ({ projects: [] })),
      ]);
      setDashboard(dash || { projects: [], total_seconds: 0 });
      setAvailableProjects(ht?.projects || []);
    } catch (error) {
      console.error(error);
    } finally {
      setLoading(false);
    }
  }, [fetchApi]);

  useEffect(() => {
    fetchApi<CurrentUser>("/api/v1/me")
      .then((user) => {
        setUserName(user.first_name);
        setHackatimeConnected(user.hackatime_connected);
        loadData();
      })
      .catch(() => {
        router.replace("/");
      });
  }, [fetchApi, loadData, router]);

  const handleCreateProject = async (e: React.FormEvent) => {
    e.preventDefault();
    setCreateError("");
    try {
      await fetchApi("/api/v1/projects", {
        method: "POST",
        body: JSON.stringify({ title, description }),
      });
      setTitle("");
      setDescription("");
      await loadData();
    } catch (error) {
      setCreateError(errorMessage(error));
    }
  };

  const handleLogout = async () => {
    try {
      await fetchApi("/auth/logout", { method: "POST" });
    } finally {
      router.replace("/");
    }
  };

  const openModal = (proj: Project) => {
    setSelectedProject(proj);
    setSelectedNames(proj.linked_project_names || []);
    setLinkError("");
  };

  const handleSaveLinks = async () => {
    if (!selectedProject) return;
    setLinkError("");
    try {
      await fetchApi(`/api/v1/projects/${selectedProject.id}/hackatime-projects`, {
        method: "PUT",
        body: JSON.stringify({ names: selectedNames }),
      });
      setSelectedProject(null);
      await loadData();
    } catch (error) {
      setLinkError(errorMessage(error));
    }
  };

  const formatTime = (seconds: number) => {
    const s = Math.round(seconds || 0);
    const hours = Math.floor(s / 3600);
    const minutes = Math.floor((s % 3600) / 60);
    return `${hours}h ${minutes}m`;
  };

  return (
    <div className="min-h-screen bg-slate-50 text-slate-900">
      <header className="max-w-6xl mx-auto h-20 flex items-center justify-between px-6 border-b border-slate-200/60">
        <Link href="/dashboard" className="font-mono text-xl font-extrabold tracking-tight">
          test-instance<span className="text-[#ec3750]">_</span>
        </Link>
        <div className="flex items-center gap-4 text-sm font-semibold">
          <span>{userName}</span>
          <button
            onClick={handleLogout}
            className="text-slate-500 hover:text-[#ec3750] transition font-bold text-xs"
          >
            Log out
          </button>
        </div>
      </header>

      <main className="max-w-6xl mx-auto px-6 py-8">
        <section className="flex flex-col md:flex-row justify-between items-start md:items-end gap-6 mb-10">
          <div>
            <p className="uppercase tracking-widest text-xs font-bold text-[#ec3750] mb-2">
              Your workbench
            </p>
            <h1 className="text-3xl md:text-5xl font-extrabold tracking-tight leading-tight max-w-2xl">
              Build in public. Track every minute.
            </h1>
          </div>
          <div className="w-full md:w-auto p-5 rounded-2xl border border-[#ec3750]/20 bg-gradient-to-br from-[#ec3750]/5 to-[#ec3750]/15 min-w-[240px]">
            <span className="text-xs font-semibold text-slate-500">Total tracked</span>
            <strong className="block text-4xl font-extrabold text-[#ec3750] my-1">
              {formatTime(dashboard.total_seconds)}
            </strong>
            <small className="text-xs text-slate-500">across your linked projects</small>
          </div>
        </section>

        {!hackatimeConnected && (
          <section className="p-5 rounded-2xl bg-emerald-50 border border-emerald-200 flex flex-col md:flex-row items-start md:items-center justify-between gap-4 mb-8">
            <div>
              <strong className="text-emerald-950 text-base">Connect Hackatime to start tracking.</strong>
              <p className="text-xs text-emerald-800 mt-1">
                Link the projects you code on, then your time appears here automatically.
              </p>
            </div>
            <Link
              href="/auth/hackatime/login"
              className="px-4 py-2.5 bg-[#ec3750] hover:bg-[#d62740] text-white text-xs font-bold rounded-xl shadow transition"
            >

              Connect Hackatime
            </Link>
          </section>
        )}

        <div className="grid grid-cols-1 md:grid-cols-[340px_1fr] gap-8 items-start">
          <aside className="bg-[#17171d] text-white p-7 rounded-2xl shadow-xl sticky top-6">
            <p className="uppercase tracking-widest text-xs font-bold text-[#ec3750] mb-2">
              New project
            </p>
            <h2 className="text-2xl font-extrabold mb-5">Make something real.</h2>
            <form onSubmit={handleCreateProject} className="flex flex-col gap-4">
              <div>
                <label className="block text-xs font-bold text-slate-300 mb-1">Project name</label>
                <input
                  value={title}
                  onChange={(e) => setTitle(e.target.value)}
                  maxLength={120}
                  required
                  placeholder="A little something"
                  className="w-full px-3.5 py-2.5 bg-slate-900 border border-slate-700 rounded-xl text-white focus:outline-none focus:border-[#ec3750]"
                />
              </div>
              <div>
                <label className="block text-xs font-bold text-slate-300 mb-1">
                  What are you making? <span className="font-normal text-slate-400">optional</span>
                </label>
                <textarea
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  maxLength={500}
                  placeholder="A short description"
                  className="w-full px-3.5 py-2.5 bg-slate-900 border border-slate-700 rounded-xl text-white focus:outline-none focus:border-[#ec3750] min-h-[100px]"
                />
              </div>
              {createError && <p className="text-red-400 text-xs">{createError}</p>}
              <button
                type="submit"
                className="w-full py-3 px-4 bg-[#ec3750] hover:bg-[#d62740] text-white font-bold rounded-xl shadow transition mt-1"
              >
                Create project
              </button>
            </form>
          </aside>

          <section>
            <div className="flex justify-between items-end mb-5">
              <div>
                <p className="uppercase tracking-widest text-xs font-bold text-[#ec3750] mb-1">
                  Projects
                </p>
                <h2 className="text-2xl font-extrabold">Your current constellation</h2>
              </div>
              <button
                onClick={loadData}
                className="text-xs font-bold text-slate-500 hover:text-[#ec3750] transition"
              >
                Refresh time
              </button>
            </div>

            {loading ? (
              <p className="text-slate-400 text-sm">Loading your projects…</p>
            ) : dashboard.projects.length === 0 ? (
              <div className="p-10 text-center border-2 border-dashed border-slate-200 rounded-2xl text-slate-400 font-medium">
                No projects yet. Make the first one.
              </div>
            ) : (
              <div className="grid gap-4">
                {dashboard.projects.map((proj) => (
                  <article
                    key={proj.id}
                    className="p-6 bg-white border border-slate-200/80 rounded-2xl shadow-sm flex flex-col md:flex-row justify-between gap-4 hover:shadow-md transition"
                  >
                    <div>
                      <h3 className="text-xl font-bold mb-1">{proj.title}</h3>
                      {proj.description && (
                        <p className="text-slate-500 text-sm mb-3">{proj.description}</p>
                      )}
                      <p className="text-xs font-semibold text-slate-600">
                        {proj.linked_project_names.length
                          ? `Tracking: ${proj.linked_project_names.join(", ")}`
                          : "No Hackatime projects linked yet"}
                      </p>
                    </div>
                    <div className="md:text-right flex flex-col justify-between items-start md:items-end">
                      <div>
                        <strong className="block text-2xl font-extrabold">
                          {formatTime(proj.total_seconds)}
                        </strong>
                        <span className="text-xs text-slate-400">tracked time</span>
                      </div>
                      <button
                        onClick={() => openModal(proj)}
                        className="mt-3 px-3 py-1.5 bg-[#ec3750]/10 hover:bg-[#ec3750]/20 text-[#d62740] font-bold text-xs rounded-lg transition"
                      >
                        Link Hackatime
                      </button>
                    </div>
                  </article>
                ))}
              </div>
            )}
          </section>
        </div>
      </main>

      {/* Link Hackatime Modal */}
      {selectedProject && (
        <div className="fixed inset-0 bg-slate-900/60 backdrop-blur-sm flex items-center justify-center p-4 z-50">
          <div className="bg-white rounded-2xl p-6 w-full max-w-md shadow-2xl">
            <div className="flex justify-between items-start mb-2">
              <div>
                <p className="uppercase tracking-widest text-xs font-bold text-[#ec3750]">
                  Hackatime
                </p>
                <h2 className="text-xl font-extrabold">Link time to {selectedProject.title}</h2>
              </div>
              <button
                onClick={() => setSelectedProject(null)}
                className="text-slate-400 hover:text-slate-700 text-xl font-bold"
              >
                ×
              </button>
            </div>
            <p className="text-xs text-slate-500 mb-4">
              Select the Hackatime projects that belong to this project.
            </p>

            <div className="max-h-60 overflow-y-auto border-t border-b border-slate-100 py-2 my-4">
              {availableProjects.length === 0 ? (
                <p className="text-xs text-slate-400 py-4 text-center">
                  No Hackatime projects found. Code on a project first, then refresh.
                </p>
              ) : (
                availableProjects.map((hp) => {
                  const checked = selectedNames.includes(hp.name);
                  return (
                    <label
                      key={hp.name}
                      className="flex items-center gap-3 py-2.5 px-2 hover:bg-slate-50 rounded-lg cursor-pointer text-sm font-semibold"
                    >
                      <input
                        type="checkbox"
                        checked={checked}
                        onChange={(e) => {
                          if (e.target.checked) {
                            setSelectedNames([...selectedNames, hp.name]);
                          } else {
                            setSelectedNames(selectedNames.filter((n) => n !== hp.name));
                          }
                        }}
                        className="accent-[#ec3750]"
                      />
                      <span>
                        {hp.name}{" "}
                        <small className="text-slate-400 font-normal ml-1">
                          ({formatTime(hp.total_duration || 0)})
                        </small>
                      </span>
                    </label>
                  );
                })
              )}
            </div>

            {linkError && <p className="text-red-500 text-xs mb-3">{linkError}</p>}

            <div className="flex justify-end gap-3">
              <button
                onClick={() => setSelectedProject(null)}
                className="px-4 py-2 bg-slate-100 hover:bg-slate-200 text-slate-700 text-xs font-bold rounded-xl transition"
              >
                Cancel
              </button>
              <button
                onClick={handleSaveLinks}
                className="px-4 py-2 bg-[#ec3750] hover:bg-[#d62740] text-white text-xs font-bold rounded-xl shadow transition"
              >
                Save links
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
