import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { backendUrl } from "@/lib/backend";

export default async function DashboardLayout({ children }: { children: React.ReactNode }) {
  const session = (await cookies()).get("session")?.value;
  if (!session) redirect("/");

  const response = await fetch(`${backendUrl}/api/v1/me`, {
    headers: { cookie: `session=${session}` },
    cache: "no-store",
  });
  if (!response.ok) redirect("/");

  return children;
}
