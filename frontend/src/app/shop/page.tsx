"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";

type ShopItem = {
  id: string;
  slug: string;
  name: string;
  description: string;
  price_hours: number;
};

type Account = {
  available_hours: number;
  purchases: { id: string; item_id: string; item_name: string; created_at: string }[];
};

function randomIdempotencyKey() {
  return crypto.randomUUID().replaceAll("-", "");
}

export default function ShopPage() {
  const router = useRouter();
  const [items, setItems] = useState<ShopItem[]>([]);
  const [account, setAccount] = useState<Account | null>(null);
  const [message, setMessage] = useState("");
  const [claiming, setClaiming] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const request = useCallback(async <T,>(path: string, init: RequestInit = {}): Promise<T> => {
    const response = await fetch(path, { credentials: "include", ...init });
    if (!response.ok) {
      if (response.status === 401) {
        throw new Error("Unauthorized");
      }
      const body: unknown = await response.json().catch(() => null);
      const error = typeof body === "object" && body !== null && "error" in body && typeof body.error === "string"
        ? body.error : "Request failed";
      throw new Error(error);
    }
    return response.json() as Promise<T>;
  }, []);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const catalogue = await request<ShopItem[]>("/api/v1/shop/items");
      setItems(catalogue);
      
      try {
        const wallet = await request<Account>("/api/v1/shop/me");
        setAccount(wallet);
      } catch (error) {
        // If wallet fails (e.g. 401), we still show the catalogue
        if (error instanceof Error && error.message !== "Unauthorized") {
          setMessage(error.message);
        }
      }
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Unable to load shop");
    } finally {
      setLoading(false);
    }
  }, [request]);

  useEffect(() => {
    void load();
  }, [load]);

  async function claim(item: ShopItem) {
    if (!account) {
      router.push("/sign-in");
      return;
    }
    setClaiming(item.id);
    setMessage("");
    try {
      await request(`/api/v1/shop/items/${item.id}/purchase`, {
        method: "POST",
        headers: { "Idempotency-Key": randomIdempotencyKey() },
      });
      setMessage("Your ticket is confirmed. Check your inbox for the confirmation email!");
      await load();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Unable to claim item");
    } finally {
      setClaiming(null);
    }
  }

  const owned = new Set(account?.purchases.map((purchase) => purchase.item_id) ?? []);
  const hours = account?.available_hours ?? 0;
  const isAuthenticated = account !== null;

  return (
    <main className="min-h-screen bg-slate-50 text-slate-900">
      <header className="max-w-4xl mx-auto h-20 flex items-center justify-between px-6 border-b border-slate-200/60">
        <Link href="/dashboard" className="font-mono text-xl font-extrabold tracking-tight">
          test-instance<span className="text-[#ec3750]">_</span>
        </Link>
        <Link href="/dashboard" className="text-xs font-bold text-slate-500 hover:text-[#ec3750]">← Dashboard</Link>
      </header>
      <section className="max-w-4xl mx-auto px-6 py-12">
        <p className="uppercase tracking-widest text-xs font-bold text-[#ec3750] mb-2">Approved-hours shop</p>
        <div className="flex flex-col sm:flex-row justify-between gap-6 items-start mb-10">
          <div>
            <h1 className="text-4xl font-extrabold tracking-tight">Claim your event ticket.</h1>
            <p className="mt-2 text-slate-500">Projects earn credit only after both reviewer and fraud approval.</p>
          </div>
          {isAuthenticated ? (
            <div className="rounded-2xl border border-[#ec3750]/20 bg-[#ec3750]/5 px-5 py-4 min-w-44">
              <span className="text-xs font-semibold text-slate-500">Available balance</span>
              <strong className="block text-3xl text-[#ec3750]">{hours.toFixed(2)}h</strong>
            </div>
          ) : (
            <div className="rounded-2xl border border-slate-200 bg-white px-5 py-4 min-w-44">
              <span className="text-xs font-semibold text-slate-500">Available balance</span>
              <Link href="/sign-in" className="block text-sm font-bold text-[#ec3750] mt-1 hover:underline">Sign in to check →</Link>
            </div>
          )}
        </div>
        {message && <p className="mb-5 rounded-xl border border-slate-200 bg-white p-4 text-sm font-semibold">{message}</p>}
        {loading ? (
           <p className="text-slate-400 text-sm">Loading items…</p>
        ) : (
          <div className="grid gap-5 sm:grid-cols-2">
            {items.map((item) => {
              const isOwned = owned.has(item.id);
              const canClaim = hours >= item.price_hours && !isOwned;
              return (
                <article key={item.id} className="rounded-2xl border border-slate-200 bg-white p-6 shadow-sm">
                  <p className="text-xs font-bold uppercase tracking-widest text-[#ec3750]">{item.price_hours.toFixed(0)} approved hours</p>
                  <h2 className="mt-2 text-2xl font-extrabold">{item.name}</h2>
                  <p className="mt-2 min-h-12 text-sm text-slate-500">{item.description}</p>
                  <button
                    onClick={() => void claim(item)}
                    disabled={isAuthenticated && (!canClaim || claiming === item.id)}
                    className="mt-6 w-full rounded-xl bg-[#ec3750] px-4 py-3 text-sm font-bold text-white hover:bg-[#d62740] disabled:cursor-not-allowed disabled:bg-slate-300"
                  >
                    {!isAuthenticated ? "Sign in to claim" : isOwned ? "Already claimed" : claiming === item.id ? "Confirming…" : canClaim ? "Claim ticket" : `Need ${(item.price_hours - hours).toFixed(2)} more hours`}
                  </button>
                </article>
              );
            })}
          </div>
        )}
      </section>
    </main>
  );
}
