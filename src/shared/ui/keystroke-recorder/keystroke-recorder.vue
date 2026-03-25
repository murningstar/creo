<template>
    <div>
        <div
            ref="containerRef"
            tabindex="0"
            class="flex min-h-9 cursor-pointer items-center justify-center gap-1 rounded-lg border px-3 py-1.5 text-sm transition-colors outline-none"
            :class="containerClasses"
            @focus="onFocus"
            @blur="onBlur"
            @keydown.prevent="onKeyDown"
            @keyup="onKeyUp"
        >
            <template v-if="recording">
                <span class="text-muted animate-pulse text-xs">Press a key combination...</span>
            </template>
            <template v-else-if="displayKeys.length > 0">
                <template v-for="(key, idx) in displayKeys" :key="idx">
                    <u-kbd size="lg">{{ key }}</u-kbd>
                    <span v-if="idx < displayKeys.length - 1" class="text-dimmed">+</span>
                </template>
            </template>
            <template v-else>
                <span class="text-dimmed text-xs">Click to record shortcut</span>
            </template>
        </div>
        <p v-if="!recording" class="text-dimmed mt-0.5 text-xs">
            <slot name="hint">Click to change hotkey</slot>
        </p>
    </div>
</template>

<script setup lang="ts">
    import type { KeyCombo } from './model/types';

    const props = withDefaults(
        defineProps<{
            modelValue?: KeyCombo | null;
        }>(),
        {
            modelValue: null,
        }
    );

    const emit = defineEmits<{
        'update:modelValue': [combo: KeyCombo];
        'recording-start': [];
        'recording-end': [];
        cancelled: [];
    }>();

    const containerRef = ref<HTMLElement | null>(null);
    const recording = ref(false);

    // Track currently held modifier keys for visual feedback
    const heldModifiers = ref<Set<string>>(new Set());

    const displayKeys = computed(() => {
        if (recording.value && heldModifiers.value.size > 0) {
            return [...heldModifiers.value];
        }
        if (!props.modelValue) return [];
        return formatCombo(props.modelValue);
    });

    const containerClasses = computed(() => {
        if (recording.value) {
            return 'border-primary bg-primary/5 ring-2 ring-primary/20';
        }
        return 'border-default bg-neutral-100 dark:bg-neutral-800 hover:border-neutral-400 dark:hover:border-neutral-500';
    });

    function formatCombo(combo: KeyCombo): string[] {
        const parts: string[] = [];
        if (combo.ctrl) parts.push('Ctrl');
        if (combo.alt) parts.push('Alt');
        if (combo.shift) parts.push('Shift');
        if (combo.meta) parts.push('Super');
        if (combo.key) parts.push(prettifyKey(combo.key, combo.code));
        return parts;
    }

    function prettifyKey(key: string, code: string): string {
        // Map common key names to display-friendly labels
        const keyMap: Record<string, string> = {
            Backquote: '`',
            Backslash: '\\',
            BracketLeft: '[',
            BracketRight: ']',
            Comma: ',',
            Period: '.',
            Slash: '/',
            Semicolon: ';',
            Quote: "'",
            Minus: '-',
            Equal: '=',
            Space: 'Space',
            Enter: 'Enter',
            Backspace: 'Backspace',
            Tab: 'Tab',
            Escape: 'Esc',
            Delete: 'Delete',
            Insert: 'Insert',
            Home: 'Home',
            End: 'End',
            PageUp: 'PgUp',
            PageDown: 'PgDn',
            ArrowUp: '↑',
            ArrowDown: '↓',
            ArrowLeft: '←',
            ArrowRight: '→',
            ScrollLock: 'Scroll Lock',
            Pause: 'Pause Break',
            PrintScreen: 'Print Screen',
            NumLock: 'Num Lock',
            CapsLock: 'Caps Lock',
        };

        // F-keys
        if (code.startsWith('F') && /^F\d+$/.test(code)) return code;

        // Named code mapping
        if (code in keyMap) return keyMap[code]!;

        // Numpad keys
        if (code.startsWith('Numpad')) return `Num ${code.replace('Numpad', '')}`;

        // Letter/digit keys from code (e.g., KeyA → A, Digit1 → 1)
        if (code.startsWith('Key')) return code.replace('Key', '');
        if (code.startsWith('Digit')) return code.replace('Digit', '');

        // Fallback to key value
        return key.length === 1 ? key.toUpperCase() : key;
    }

    const MODIFIER_KEYS = new Set(['Control', 'Alt', 'Shift', 'Meta']);

    function modifierLabel(key: string): string {
        const map: Record<string, string> = {
            Control: 'Ctrl',
            Alt: 'Alt',
            Shift: 'Shift',
            Meta: 'Super',
        };
        return map[key] ?? key;
    }

    function onFocus() {
        recording.value = true;
        heldModifiers.value = new Set();
        emit('recording-start');
    }

    function onBlur() {
        recording.value = false;
        heldModifiers.value = new Set();
        emit('recording-end');
    }

    function onKeyDown(e: KeyboardEvent) {
        if (MODIFIER_KEYS.has(e.key)) {
            heldModifiers.value = new Set([...heldModifiers.value, modifierLabel(e.key)]);
            return;
        }

        // Escape cancels recording without changing the hotkey
        if (e.code === 'Escape') {
            emit('cancelled');
            containerRef.value?.blur(); // onBlur handles cleanup + recording-end
            return;
        }

        // Non-modifier key pressed — emit the combo, blur handles the rest
        emit('update:modelValue', {
            key: e.key,
            code: e.code,
            ctrl: e.ctrlKey,
            alt: e.altKey,
            shift: e.shiftKey,
            meta: e.metaKey,
        });
        containerRef.value?.blur();
    }

    function onKeyUp(e: KeyboardEvent) {
        if (MODIFIER_KEYS.has(e.key)) {
            const next = new Set(heldModifiers.value);
            next.delete(modifierLabel(e.key));
            heldModifiers.value = next;
        }
    }
</script>
