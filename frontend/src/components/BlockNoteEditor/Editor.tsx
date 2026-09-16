"use client";

import { useEffect } from "react";
import type { PartialBlock, Block } from "@blocknote/core";
import { useCreateBlockNote } from "@blocknote/react";
import { BlockNoteView } from "@blocknote/shadcn";
import "@blocknote/shadcn/style.css";
import "@blocknote/core/fonts/inter.css";
import { useTheme } from "next-themes";

interface EditorProps {
  initialContent?: Block[];
  onChange?: (blocks: Block[]) => void;
  editable?: boolean;
}

export default function Editor({ initialContent, onChange, editable = true }: EditorProps) {
  console.log('📝 EDITOR: Initializing BlockNote editor with blocks:', {
    hasContent: !!initialContent,
    blocksCount: initialContent?.length || 0,
    editable
  });

  const { resolvedTheme } = useTheme();
  const editor = useCreateBlockNote({
    initialContent: initialContent as PartialBlock[] | undefined,
  });

  console.log('📝 EDITOR: BlockNote editor created successfully');

  // Handle content changes
  useEffect(() => {
    if (!onChange) return;

    const handleChange = () => {
      console.log('📝 EDITOR: Content changed, notifying parent...', {
        blocksCount: editor.document.length
      });
      onChange(editor.document);
    };

    const unsubscribe = editor.onChange(handleChange);

    return () => {
      if (typeof unsubscribe === 'function') {
        console.log('📝 EDITOR: Cleaning up onChange listener');
        unsubscribe();
      }
    };
  }, [editor, onChange]);

  // The editor used to be pinned to `theme="light"`, so in dark mode a summary opened
  // as a white sheet inside a near-black page. `next-themes` resolves `system` to the
  // actual value, which is what BlockNote needs — passing "system" through would leave
  // it light. The globals.css `!important` sizing overrides are untouched.
  return (
    <BlockNoteView
      editor={editor}
      editable={editable}
      theme={resolvedTheme === 'dark' ? 'dark' : 'light'}
    />
  );
}
