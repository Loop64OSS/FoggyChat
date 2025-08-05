import * as slider from '@zag-js/slider';
import type { Snippet } from 'svelte';
export interface SliderProps extends Omit<slider.Props, 'id' | 'thumbSize'> {
    /** Provide an array of value markers */
    markers?: number[];
    /** Set height classes for the overall slider. */
    height?: string;
    /** Set base classes. */
    base?: string;
    /** Set vertical spacing between the control and markers. */
    spaceY?: string;
    /** Provide arbitrary classes for the root. */
    classes?: string;
    /** Set base classes for the control. */
    controlBase?: string;
    /** Provide arbitrary classes for the control. */
    controlClasses?: string;
    /** Set base classes for the track. */
    trackBase?: string;
    /** Set background classes for the track. */
    trackBg?: string;
    /** Set border radius classes for the track. */
    trackRounded?: string;
    /** Provide arbitrary classes for the track. */
    trackClasses?: string;
    /** Set base classes for the meter. */
    meterBase?: string;
    /** Set background classes for the meter. */
    meterBg?: string;
    /** Set border radius classes for the meter. */
    meterRounded?: string;
    /** Provide arbitrary classes for the meter. */
    meterClasses?: string;
    /** Set base classes for the thumb. */
    thumbBase?: string;
    /** Set size classes for the thumb. */
    thumbSize?: string;
    /** Set background classes for the thumb. */
    thumbBg?: string;
    /** Set ring size classes for the thumb. */
    thumbRingSize?: string;
    /** Set ring color classes for the thumb. */
    thumbRingColor?: string;
    /** Set border-radius classes for the thumb. */
    thumbRounded?: string;
    /** Set cursor classes for the thumb. */
    thumbCursor?: string;
    /** Provide arbitrary classes for the thumb. */
    thumbClasses?: string;
    /** Set base classes for the markers. */
    markersBase?: string;
    /** Provide arbitrary classes for the markers. */
    markersClasses?: string;
    /** Set base classes for each mark. */
    markBase?: string;
    /** Set text size classes for each mark. */
    markText?: string;
    /** Set opacity classes for each mark. */
    markOpacity?: string;
    /** Provide arbitrary classes for each mark. */
    markClasses?: string;
    /** Set disabled state classes for the root element. */
    stateDisabled?: string;
    /** Set read-only state classes for the root element. */
    stateReadOnly?: string;
    /** Replace numeric markers with symbol, such as a icon. Takes marker value as argument. */
    mark?: Snippet<[number]>;
}
