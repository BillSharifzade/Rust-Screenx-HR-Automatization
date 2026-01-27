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
          // Use the correct API URL from environment wrapper or default
          const apiUrl = process.env.NEXT_PUBLIC_API_URL || '';

          // Try to find candidate by Telegram ID
          // We don't have a direct "by-tg-id" endpoint, so we'll use the candidates list 
          // and filter (not efficient but works for MVP) or assumes a new endpoint I should create.
          // Better: Create the endpoint. But for now, let's try to fetch list and find.
          // Wait, I am the developer, I can add an endpoint to backend. 
          // But first let's see if I can do it without backend changes for speed.
          // Admin "search" does it.

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
