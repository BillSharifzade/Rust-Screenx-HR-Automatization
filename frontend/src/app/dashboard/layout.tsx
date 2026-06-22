import { Sidebar } from "@/components/sidebar";
import { NotificationsProvider } from "@/lib/notifications-context";
import { AuthProvider } from "@/lib/auth-context";
import { AuthGuard } from "@/components/auth-guard";

export default function DashboardLayout({
    children,
}: {
    children: React.ReactNode;
}) {
    return (
        <AuthProvider>
            <AuthGuard>
                <NotificationsProvider>
                    <div className="flex min-h-screen w-full">
                        <Sidebar />
                        <div className="flex flex-col flex-1 bg-muted/20 min-w-0 transition-all duration-300 ease-in-out">
                            <main className="flex flex-1 flex-col gap-6 p-6 lg:gap-8 lg:p-8">
                                {children}
                            </main>
                        </div>
                    </div>
                </NotificationsProvider>
            </AuthGuard>
        </AuthProvider>
    );
}
