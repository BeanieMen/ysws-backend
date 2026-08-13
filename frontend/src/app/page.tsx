"use client";

import { useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";

export default function Home() {
  const [email, setEmail] = useState("");
  const [error, setError] = useState("");
  const router = useRouter();

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = email.trim().toLowerCase();
    if (!trimmed || !trimmed.includes("@")) {
      setError("Please enter a valid email address.");
      return;
    }
    router.push(`/sign-in?email=${encodeURIComponent(trimmed)}`);
  };

  return (
    <main className="min-h-screen grid place-items-center p-6 bg-[radial-gradient(circle_at_85%_15%,rgba(236,55,80,0.08)_0%,transparent_40%),radial-gradient(circle_at_15%_85%,rgba(59,130,246,0.08)_0%,transparent_40%)]">
      <div className="w-full max-w-[460px] p-10 bg-white/80 backdrop-blur-md border border-slate-200/80 rounded-3xl shadow-xl">
        <Link href="/" className="font-mono text-xl font-extrabold text-[#17171d] tracking-tight">
          test-instance<span className="text-[#ec3750]">_</span>
        </Link>
        <p className="uppercase tracking-widest text-xs font-bold text-[#ec3750] mt-7 mb-2">
          Project time, kept honest
        </p>
        <h1 className="text-3xl font-extrabold text-slate-900 mb-2">What’s your email?</h1>
        <p className="text-slate-500 text-sm mb-6">We’ll use it to connect your Hack Club account.</p>

        <form onSubmit={handleSubmit} className="flex flex-col gap-3">
          <label htmlFor="email" className="text-xs font-bold text-slate-700">
            Email address
          </label>
          <input
            id="email"
            type="email"
            placeholder="you@example.com"
            value={email}
            onChange={(e) => {
              setEmail(e.target.value);
              setError("");
            }}
            autoFocus
            required
            className="w-full px-3.5 py-3 border border-slate-200 rounded-xl bg-white text-slate-900 focus:outline-none focus:border-[#ec3750] focus:ring-4 focus:ring-[#ec3750]/15 transition"
          />
          {error && <p className="text-red-500 text-xs font-medium">{error}</p>}
          <button
            type="submit"
            className="w-full mt-2 py-3 px-5 bg-[#ec3750] hover:bg-[#d62740] text-white font-bold rounded-xl shadow-md hover:shadow-lg transition flex items-center justify-center gap-2"
          >
            Continue <span>→</span>
          </button>
        </form>
        <p className="text-center text-xs text-slate-400 mt-6">
          Use the email connected to your Hack Club account.
        </p>
      </div>
    </main>
  );
}
