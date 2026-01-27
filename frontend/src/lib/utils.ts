import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"
import React from "react"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/**
 * Formats text by converting escaped newline sequences to actual newlines.
 * Handles both \\n (literal backslash-n) and \n (actual newline).
 */
export function formatText(text: string | null | undefined): string {
  if (!text) return '';
  // Replace literal \\n sequences with actual newlines
  return text
    .replace(/\\n/g, '\n')
    .replace(/\\r/g, '')
    .trim();
}

/**
 * React component that renders formatted text with proper line breaks.
 * Converts \\n to actual line breaks and renders as paragraphs or line breaks.
 */
export function FormattedText({
  text,
  className = '',
  as = 'div'
}: {
  text: string | null | undefined;
  className?: string;
  as?: 'div' | 'p' | 'span';
}) {
  if (!text) return null;

  const formatted = formatText(text);
  const lines = formatted.split('\n');

  const Component = as;

  return React.createElement(
    Component,
    { className: cn('whitespace-pre-wrap', className) },
    lines.map((line, i) => (
      React.createElement(React.Fragment, { key: i },
        line,
        i < lines.length - 1 ? React.createElement('br') : null
      )
    ))
  );
}
