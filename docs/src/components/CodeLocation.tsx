import React from 'react';

interface CodeLocationProps {
  file: string;
  line?: number;
  children?: React.ReactNode;
}

/**
 * Renders a clickable file location reference like src/types.rs:42
 */
export default function CodeLocation({ file, line, children }: CodeLocationProps) {
  const display = line ? `${file}:${line}` : file;
  const href = `https://github.com/XUranus/vpt-rs/blob/master/${file}${line ? `#L${line}` : ''}`;

  return (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      style={{
        fontFamily: 'monospace',
        background: 'var(--ifm-color-emphasis-100)',
        padding: '0.15rem 0.4rem',
        borderRadius: '4px',
        fontSize: '0.9em',
        textDecoration: 'none',
        color: 'var(--ifm-color-primary)',
        border: '1px solid var(--ifm-color-emphasis-300)',
      }}
    >
      {children || display}
    </a>
  );
}
