import type { Snippet } from 'svelte';
import * as accordion from '@zag-js/accordion';
import type { SlideParams } from 'svelte/transition';
export interface AccordionContext extends Pick<AccordionProps, 'animationConfig' | 'iconOpen' | 'iconClosed'> {
    api: ReturnType<typeof accordion.connect>;
}
export interface AccordionProps extends Omit<accordion.Props, 'id' | 'orientation'> {
    /** The animation configuration */
    animationConfig?: SlideParams;
    /** Sets base styles. */
    base?: string;
    /** Set padding styles. */
    padding?: string;
    /** Set vertical spacing styles. */
    spaceY?: string;
    /** Set border radius styles. */
    rounded?: string;
    /** Set the width styles. */
    width?: string;
    /** Provide arbitrary CSS classes. */
    classes?: string;
    /** The default child slot. */
    children: Snippet;
    /** Set the open state icon. */
    iconOpen?: Snippet;
    /** Set the closed state icon. */
    iconClosed?: Snippet;
}
export interface AccordionItemProps extends accordion.ItemProps {
    /** The heading level. */
    headingLevel?: number;
    /** Sets base styles. */
    base?: string;
    /** Set vertical spacing styles. */
    spaceY?: string;
    /** Provide arbitrary CSS classes. */
    classes?: string;
    /** Sets control's base styles. */
    controlBase?: string;
    /** Sets control's the hover styles. */
    controlHover?: string;
    /** Sets control's the padding styles. */
    controlPadding?: string;
    /** Sets control's the border radius styles. */
    controlRounded?: string;
    /** Provide arbitrary CSS classes to the control. */
    controlClasses?: string;
    /** Sets the lead's base styles */
    leadBase?: string;
    /** Provide arbitrary CSS classes to the lead. */
    leadClasses?: string;
    /** Sets the lead's base styles */
    contentBase?: string;
    /** Provide arbitrary CSS classes to the content. */
    contentClasses?: string;
    /** Sets the lead's base styles */
    indicatorBase?: string;
    /** Provide arbitrary CSS classes to the indicator. */
    indicatorClasses?: string;
    /** Set the panel's base styles. */
    panelBase?: string;
    /** Set the panel's padding styles. */
    panelPadding?: string;
    /** Set the panel's border-radius styles. */
    panelRounded?: string;
    /** Provide arbitrary CSS classes to the panel. */
    panelClasses?: string;
    /** The control's default slot. */
    control: Snippet;
    /** The control's lead icon slot. */
    lead?: Snippet;
    /** The panels's default slot. */
    panel?: Snippet;
}
