'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';

export default function Home() {
  const router = useRouter();
  const [isChecking, setIsChecking] = useState(true);

  useEffect(() => {
    const checkUser = async () => {
      // @ts-ignore
      const tg = window?.Telegram?.WebApp;

      if (tg?.initDataUnsafe?.user) {
        try {
          const apiUrl = process.env.NEXT_PUBLIC_API_URL || '';
          const tgId = tg.initDataUnsafe.user.id;
          const res = await fetch(`${apiUrl}/api/integration/candidates`);
          if (res.ok) {
            const candidates = await res.json();
            const found = candidates.find((c: any) => c.telegram_id == tgId);

            if (found) {
              router.replace(`/candidate/${found.id}`);
              return;
            }
          }

          // Not found -> Register
          router.replace('/candidate/register');

        } catch (e) {
          console.error(e);
          router.replace('/dashboard');
        }
      } else {
        // Not in Telegram -> Admin Dashboard
        router.replace('/dashboard');
      }
      setIsChecking(false);
    };

    checkUser();
  }, [router]);

  return (
    <div className="flex items-center justify-center min-h-screen">
      <div className="animate-spin h-8 w-8 border-4 border-primary border-t-transparent rounded-full" />
    </div>
  );
}
