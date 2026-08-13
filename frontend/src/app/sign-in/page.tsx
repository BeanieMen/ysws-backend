"use client";

import { useSearchParams, useRouter } from "next/navigation";
import Link from "next/link";
import { Suspense } from "react";

function SignInContent() {
  const searchParams = useSearchParams();
  const email = searchParams.get("email");
  const router = useRouter();

  if (!email) {
    if (typeof window !== "undefined") {
      router.replace("/");
    }
    return null;
  }

  const loginUrl = `/auth/hackclub/login?email=${encodeURIComponent(email)}`;


  return (
    <main className="min-h-screen grid place-items-center p-6 bg-[radial-gradient(circle_at_85%_15%,rgba(236,55,80,0.08)_0%,transparent_40%),radial-gradient(circle_at_15%_85%,rgba(59,130,246,0.08)_0%,transparent_40%)]">
      <div className="w-full max-w-[460px] p-10 bg-white/80 backdrop-blur-md border border-slate-200/80 rounded-3xl shadow-xl">
        <Link href="/" className="font-mono text-xl font-extrabold text-[#17171d] tracking-tight">
          test-instance<span className="text-[#ec3750]">_</span>
        </Link>
        <p className="uppercase tracking-widest text-xs font-bold text-[#ec3750] mt-7 mb-2">
          One more thing
        </p>
        <h1 className="text-3xl font-extrabold text-slate-900 mb-2">Sign in with Hack Club</h1>
        <p className="text-slate-500 text-sm mb-6">
          You’re continuing as <strong className="text-slate-800">{email}</strong>.
        </p>

        <a
          href={loginUrl}
          className="w-full py-3.5 px-5 bg-[#17171d] hover:bg-[#272730] text-white font-bold rounded-xl shadow-lg transition flex items-center justify-center gap-3 text-center"
        >
          <span className="w-5 h-5 bg-white text-[#17171d] font-mono text-xs font-bold rounded flex items-center justify-center">
            H
          </span>
          Sign in with Hack Club
        </a>

        <Link
          href="/"
          className="block text-center text-xs font-semibold text-slate-500 hover:text-slate-800 mt-6 transition"
        >
          ← Use a different email
        </Link>
      </div>
    </main>
  );
}

export default function SignInPage() {
  return (
    <Suspense fallback={<div className="min-h-screen grid place-items-center text-slate-500">Loading...</div>}>
      <SignInContent />
    </Suspense>
  );
}
