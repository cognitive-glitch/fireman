// Generated by cargo ts

export interface Ir {
   parentsAssemblyIndex: number;
   data: string;
}

export interface Assembly {
   index: number;
   parentsStartAddress: number;
   data: string;
}

export interface KnownSection {
   startAddress: number;
   endAddress: number | null;
   analyzed: boolean;
}

export interface DecompileResult {
   assembly: (Assembly)[];
   ir: (Ir)[];
   decompiled: string;
}
