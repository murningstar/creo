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
                root: 'w-fit min-w-96',
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
            base: 'px-2',
            defaultVariants: {
                size: 'sm',
                variant: 'outline',
            },
        },
        alert: {
            slots: {
                root: 'p-2.5 gap-2',
                title: 'text-xs',
                description: 'text-xs',
                icon: 'size-4',
            },
        },
    },
});
