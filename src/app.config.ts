export default defineAppConfig({
    ui: {
        colors: {
            info: 'creo',
        },
        tabs: {
            defaultVariants: {
                variant: 'pill',
                size: 'xs',
            },
            slots: {
                list: 'grid w-full auto-cols-fr grid-flow-col',
                trigger: 'text-center',
                root: 'w-full',
            },
            compoundVariants: [
                {
                    variant: 'pill' as const,
                    class: {
                        trigger: 'data-[state=active]:!bg-transparent data-[state=active]:text-highlighted',
                        indicator: 'bg-white shadow-sm',
                    },
                },
            ],
        },
        kbd: {
            base: 'px-2 normal-case',
            defaultVariants: {
                size: 'sm',
                variant: 'outline',
            },
        },
        alert: {
            slots: {
                root: 'p-2 gap-1.5',
                title: 'text-xs',
                description: 'text-xs',
                icon: 'size-4',
            },
            variants: {
                title: {
                    true: {
                        description: 'mt-0.5',
                    },
                },
            },
        },
    },
});
