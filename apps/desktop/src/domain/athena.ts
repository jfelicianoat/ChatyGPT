/** El area de Athena: fases, tareas, historia, memoria, perfiles y modelos. */

/**
 * Fase de un run de Athena.
 *
 * Son los estados que publica el runtime más `starting`, que es el hueco entre
 * pedir el run y recibir su primer estado. La interfaz nunca deduce ninguna.
 */
export type AthenaFase =
  | "starting"
  | "running"
  | "waiting_permission"
  | "verifying"
  | "completed"
  | "failed"
  /**
   * El trabajo termino y no se pudo comprobar: sin checks que ejecutar, con una
   * dependencia que falta o con el entorno a medias. No es haber fallado, y
   * contarlo igual le echa la culpa al cambio de una maquina rota.
   */
  | "unverified"
  | "cancelled"
  | "recovery_pending";

/** Estado de una tarea del TaskManager de Athena. */
export type AthenaEstadoTarea =
  | "pending"
  | "running"
  | "completed"
  | "failed"
  | "cancelled"
  | "killed"
  | "recovery_pending";

export type AthenaEstadoServicio =
  | "desconocido"
  /** Responde, habla un contrato que entendemos y nos conoce. */
  | "conectado"
  /** Responde, pero todavía no le hemos dado credencial. */
  | "sin_credencial"
  /** Responde, tenemos credencial y la rechaza: hay que volver a vincular. */
  | "credencial_invalida"
  | "no_disponible"
  | "incompatible";

/** Estado del servicio. Nunca incluye el token: ese valor no sale de Rust. */
export type AthenaEstadoArea = {
  estado: AthenaEstadoServicio;
  urlBase: string;
  credencialConfigurada: boolean;
  versionContrato?: number;
  detalle?: string;
  runsActivos: number;
};

export type AthenaTarea = {
  id: string;
  nombre: string;
  estado: AthenaEstadoTarea;
  iteraciones?: number;
  llamadasHerramienta?: number;
  detalle?: string;
  /** Especialista asignado por Athena. Vacío cuando el run no es jerárquico. */
  rol: string;
  /** Tareas de las que ésta depende, tal y como las publicó Athena. */
  dependencias: string[];
  /** Ficheros que esta tarea cambió, según su propia evidencia. */
  ficheros: string[];
};

/** Un campo suelto de un resultado estructurado, ya legible. */
/** Un hecho del registro duradero de un run, listo para enseñar en una línea. */
export type AthenaHechoHistorico = {
  /** Su sitio en el orden. Lo asigna el registro, no quien publica. */
  secuencia: number;
  nombre: string;
  cuando: string;
  /** Quién lo hizo: el run, o el delegado que lo hizo por él. */
  actor: string;
  tarea?: string;
  delegado: boolean;
};

/**
 * Lo esencial de un run según Athena, derivado de sus propios hechos.
 *
 * No se recalcula en el cliente: quien escribe los hechos es quien mejor sabe leerlos, y
 * dos lectores acabarían discrepando sin que nadie supiera cuál miente.
 */
export type AthenaResumenHistoria = {
  status: string;
  /** `direct` o `hierarchical`. */
  executedAs: string;
  /** Estado final de cada tarea del plan, por id. */
  tasks: Record<string, string>;
  /** Rol de cada delegado, por sesión. */
  delegates: Record<string, string>;
  verification: string;
  permissionRequests: number;
};

/**
 * Un run terminado, reconstruido desde el registro duradero.
 *
 * `proyeccion` sale del **mismo** lector que la vista en vivo: un run releído se lee
 * igual que se leyó cuando pasaba.
 */
export type AthenaHistoria = {
  proyeccion: AthenaRun;
  resumen: AthenaResumenHistoria;
  hechos: AthenaHechoHistorico[];
};

/**
 * Algo que Athena cree saber de un proyecto.
 *
 * Dos ejes, y no uno (ADR-031). `verificacion` dice cuánto peso ha ganado —lo que dijo
 * un modelo, lo que algo comprobó, lo que una persona respaldó— y `estado` dice si sigue
 * vigente, si otro lo reemplazó o si alguien lo retiró. Juntarlos en un solo campo haría
 * indistinguible «nadie lo ha comprobado» de «ya no vale».
 */
export type AthenaRecuerdo = {
  id: string;
  projectId: string;
  /** Comando verificado, convención, decisión de arquitectura… */
  kind: string;
  content: string;
  /** De dónde salió. Un recuerdo sin origen no se puede juzgar. */
  source: string;
  sourceReference?: string | null;
  confidence: number;
  /** `proposed`, `verified` o `user_confirmed`. */
  verificationState: string;
  scope: string;
  /** `active`, `superseded` o `forgotten`. */
  status: string;
  /** El recuerdo al que éste sustituye, si sustituye a alguno. */
  supersedes?: string | null;
  createdAt: string;
  updatedAt: string;
  /** Si pasó el plazo de su tipo. Lo calcula Athena, no la interfaz. */
  stale: boolean;
};

/**
 * Un perfil de Athena: para qué clase de trabajo sirve un run.
 *
 * Cambia dos cosas a la vez —qué herramientas existen y qué cuenta como prueba— y por eso
 * se elige al crear el run y no después (ADR-028). Athena **no publica una versión** del
 * perfil, así que aquí no hay ninguna: un número que nadie mantiene invita a confiar en
 * que subiría al cambiar el perfil, y no subiría.
 */
export type AthenaPerfil = {
  name: string;
  /** Sobre qué trabaja: un repositorio, una carpeta de documentos… */
  subject: string;
  /** Qué clase de evidencia da por buena. */
  evidence: string;
  /** Qué demuestra esa evidencia, incluido lo que no demuestra. */
  proves: string;
  /** Las herramientas que existen bajo este perfil: un filtro estructural. */
  tools: string[];
  description: string;
};

export type AthenaListadoPerfiles = {
  /** El que se usa si no se pide ninguno. */
  default: string;
  profiles: AthenaPerfil[];
};

/**
 * Un modelo entre los que este despliegue deja elegir.
 *
 * Sólo el nombre y si es el de por defecto. Athena no publica adjetivos —«rápido», «bueno
 * para código»— y escribirlos aquí sería inventar una recomendación que nadie mantiene y
 * que envejecería en cuanto cambiara el despliegue.
 */
export type AthenaModelo = {
  name: string;
  /** Si es el que corre cuando no se elige ninguno. */
  default: boolean;
};

/**
 * Los modelos ofrecidos, o una lista vacía si este Athena no ofrece elección.
 *
 * Vacío no es un error: hay despliegues que corren con un modelo fijo. La interfaz no
 * enseña selector en ese caso, que es distinto de enseñar uno vacío.
 */
export type AthenaListadoModelos = {
  default: string;
  models: AthenaModelo[];
};

/**
 * Un delegado del run: un especialista al que se le encargó una parte.
 *
 * Aparte de `AthenaTarea` a propósito. Una tarea del plan *usa* un subagente; no *es*
 * uno. Mezclarlos hacía que un run jerárquico enseñara el doble de trabajo del que había.
 *
 * Nada de esto es el razonamiento del delegado ni su conversación: Athena entrega un
 * resumen, y el transcript del hijo no sale nunca de su sesión.
 */
export type AthenaDelegado = {
  /** Sesión del delegado, que es su nombre para todo lo demás. */
  sesion: string;
  /** Sesión de quien lo encargó: el run, o la tarea que lo usa. */
  padre: string;
  /** Tarea del plan a la que pertenece, si el padre es una tarea. */
  tarea?: string;
  rol: string;
  /** Quién lo ejecuta. Vacío si el despliegue no lo publica. */
  proveedor: string;
  estado: AthenaEstadoTarea;
  encargo: string;
  /** Cierto mientras se le pueda volver a preguntar. */
  continuable: boolean;
  seguimientos: number;
  seguimientosRestantes?: number;
  /** Qué está haciendo, derivado de sus propios eventos. */
  actividad: string[];
  /** Lo que informó al terminar. Un resumen, no un transcript. */
  resumen?: string;
  ficheros: string[];
  llamadasHerramienta?: number;
  /** Por qué su tarea no puede avanzar, según el ejecutor del grafo. */
  bloqueos: string[];
  error?: AthenaError;
};

export type AthenaHecho = {
  nombre: string;
  valor: string;
};

/**
 * Cómo enseñar el resultado de una herramienta, según Athena.
 *
 * `clase` es un conjunto cerrado —`text`, `items`, `change`, `record`, `reference`— y lo
 * decide el runtime (ADR-026). La interfaz elige las palabras; no elige la forma. Antes
 * la deducía leyendo el resultado, que es lo mismo que decir que cada cliente la
 * inventaba por su cuenta.
 */
export type AthenaPresentacion = {
  clase: string;
  titulo: string;
  resumen: string;
  elementos: string[];
  hechos: AthenaHecho[];
  /** Dónde vive el cuerpo cuando no cupo en el evento. */
  referencia?: string;
};

export type AthenaHerramienta = {
  nombre: string;
  estado: string;
  correlacion?: string;
  externalizado: boolean;
  /** Ausente mientras la tool está en curso, y cuando Athena no publicó ninguna. */
  presentacion?: AthenaPresentacion;
};

/**
 * Un argumento de la herramienta, tal y como Athena decidió enseñarlo.
 *
 * El saneado ocurre en el runtime, que es quien tiene el valor original; aquí
 * solo se pinta lo que llegó.
 */
export type AthenaArgumento = {
  nombre: string;
  valor: string;
  /** Tamaño original cuando el valor viene resumido. */
  caracteres?: number;
  redactado: boolean;
  resumido: boolean;
};

export type AthenaPermiso = {
  requestId: string;
  herramienta: string;
  operacion: string;
  accion: string;
  riesgo: string;
  nivel: string;
  motivo: string;
  efectos: string[];
  recursos: string[];
  workspace: string;
  argumentos: AthenaArgumento[];
  soloLectura: boolean;
  destructivo: boolean;
  confirmado: boolean;
  segundosRestantes: number;
  /** Cierto cuando el plazo se agotó: ya no se puede responder. */
  caducado: boolean;
};

export type AthenaComprobacion = {
  nombre: string;
  paso?: boolean;
};

export type AthenaError = {
  codigo: string;
  mensaje: string;
  /**
   * Cual de los huecos fue, cuando el codigo dice que no se pudo comprobar:
   * sin checks definidos, una dependencia que falta, el entorno a medias. Sin
   * esto se sabe que no se comprobo y no se sabe que arreglar.
   */
  razon?: string;
  /**
   * El dato tipado que Athena adjunta al fallo (`ADMIN_AUTH_REQUIRED` y
   * similares). Suele ser lo unico accionable: dice si hay que renovar una
   * credencial, no solo que la peticion fue rechazada.
   */
  detalle?: string;
  recuperacion?: string;
};

export type AthenaArtefacto = {
  clave: string;
  uri: string;
  tipo: string;
  tamano: number;
};

/**
 * Proyección de un run: todo lo que el área muestra.
 *
 * Cada campo procede de un evento o de una instantánea de Athena. La interfaz
 * la pinta; no la calcula ni la completa.
 */
/**
 * Con qué estrategia decidió Athena ejecutar un run, y por qué.
 *
 * Los tres códigos —`solicitada`, `seleccionada`, `codigo`— son estables y vienen del
 * runtime; las frases están para leerse. La interfaz les pone nombre en castellano y no
 * decide nada: la decisión ya venía tomada.
 */
export type AthenaEstrategia = {
  /** Lo que se pidió: `auto`, `hierarchical` o `direct`. */
  solicitada: string;
  /** Lo que se hizo: `direct` o `hierarchical`. */
  seleccionada: string;
  /** Código estable del motivo. */
  codigo: string;
  /** El motivo efectivo, en una frase. */
  motivo: string;
  /** Lo que opinó la política: `decompose` o `decline`. Puede no coincidir con lo hecho. */
  veredictoPolitica: string;
  explicacionPolitica: string;
  /** Criterios de descomposición que el objetivo cumple. */
  criterios: string[];
  /** Señales que Athena no pudo medir y dejó en su valor neutro. */
  senalesSupuestas: string[];
};

/**
 * El encargo de un run, en su versión número `revision`.
 *
 * La revisión es lo único que impide que dos personas mirando el mismo run se pisen sin
 * enterarse (ADR-029): quien cambia el encargo dice sobre cuál escribe, y quien llegó
 * tarde recibe un conflicto en vez de sobrescribir lo que nunca vio.
 */
export type AthenaObjetivo = {
  text: string;
  revision: number;
  /** Por qué se cambió, dicho por quien lo cambió. Vacío en la primera. */
  reason: string;
  revisedAt: string;
};

/**
 * Cómo acabó un intento de revisar el encargo.
 *
 * Dos respuestas y no una respuesta con error: que otro haya escrito antes es algo que
 * pasa, no un fallo de quien lo intenta, y sólo una de las dos ramas admite volver a
 * intentarlo. Nada se reintenta solo — repetir sobre la revisión nueva es una decisión
 * de quien escribió, porque el encargo del otro puede ser incompatible con el suyo.
 */
export type AthenaRevisionObjetivo =
  | { resultado: "aceptada"; objetivo: AthenaObjetivo }
  | { resultado: "conflicto"; vigente: AthenaObjetivo };

export type AthenaRun = {
  runId: string;
  objetivo: string;
  /** Revisión del encargo. Cero mientras no se sabe: la instantánea no la trae. */
  objetivoRevision: number;
  /** Por qué se cambió el encargo la última vez. */
  motivoRevision?: string;
  /**
   * El perfil con el que se pidió este run. Vacío = el de por defecto del despliegue.
   *
   * Queda fijado al crear el run: no hay forma de cambiarlo después, porque cambiar de
   * perfil a mitad cambiaría qué cuenta como prueba y la evidencia ya reunida dejaría de
   * significar lo que decía.
   */
  perfilSolicitado: string;
  /** El identificador de espacio de trabajo de Athena, que hace de proyecto. */
  workspaceId: string;
  fase?: AthenaFase;
  carpeta: string;
  degradado: boolean;
  reanudable: boolean;
  conectado: boolean;
  suscriptor?: string;
  controla: boolean;
  tareas: AthenaTarea[];
  /** Los especialistas a los que se les encargó parte del trabajo. */
  delegados: AthenaDelegado[];
  herramientas: AthenaHerramienta[];
  permisos: AthenaPermiso[];
  comprobaciones: AthenaComprobacion[];
  verificacion?: string;
  resumenVerificacion?: string;
  ficherosModificados: string[];
  artefactos: AthenaArtefacto[];
  errores: AthenaError[];
  actividad: string[];
  evidencia: string[];
  ciclosReparacion: number;
  /** Cómo se está ejecutando este run. Ausente hasta que Athena lo dice. */
  estrategia?: AthenaEstrategia;
};

/** Forma corta de un run, para la lista de recuperación. */
export type AthenaResumenRun = {
  runId: string;
  workspaceId: string;
  status: string;
  resumable: boolean;
  degraded: boolean;
  objective: string;
  filesModified: string[];
  updatedAt: string;
};
