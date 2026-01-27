"use client"

import { Moon, Sun } from "lucide-react"
import { useTheme } from "next-themes"

import { motion } from "framer-motion"
import { Button } from "@/components/ui/button"

export function ModeToggle() {
    const { theme, setTheme } = useTheme()

    return (
        <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8 px-0 rounded-full hover:bg-primary/10 transition-colors group"
            onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
        >
            <motion.div
                className="relative flex items-center justify-center w-5 h-5 transition-colors"
                whileHover={{ rotate: 180 }}
                transition={{ duration: 0.5 }}
            >
                <Sun className="h-[1.2rem] w-[1.2rem] rotate-0 scale-100 transition-all dark:-rotate-90 dark:scale-0 absolute" />
                <Moon className="h-[1.2rem] w-[1.2rem] rotate-90 scale-0 transition-all dark:rotate-0 dark:scale-100 absolute" />
            </motion.div>
        </Button>
    )
}
