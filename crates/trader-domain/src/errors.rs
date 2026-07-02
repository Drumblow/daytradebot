use thiserror::Error;

/// Erros de dados de mercado.
#[derive(Debug, Error)]
pub enum DataError {
    #[error("provedor indisponível: {0}")]
    ProviderUnavailable(String),

    #[error("símbolo inválido: {0}")]
    InvalidSymbol(String),

    #[error("timeframe inválido: {0}")]
    InvalidTimeFrame(String),

    #[error("nenhum dado retornado para {symbol} no período solicitado")]
    NoData { symbol: String },

    #[error("timeout ao buscar dados: {0}")]
    Timeout(String),

    #[error("erro do provedor: {0}")]
    Provider(String),
}

/// Erros de broker/execução.
#[derive(Debug, Error)]
pub enum BrokerError {
    #[error("conexão com broker falhou: {0}")]
    ConnectionFailed(String),

    #[error("ordem rejeitada: {0}")]
    OrderRejected(String),

    #[error("ordem não encontrada: {0}")]
    OrderNotFound(String),

    #[error("timeout do broker: {0}")]
    Timeout(String),

    #[error("erro interno do broker: {0}")]
    Internal(String),
}

/// Erros de repositório/persistência.
#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("erro de conexão: {0}")]
    Connection(String),

    #[error("erro de query: {0}")]
    Query(String),

    #[error("conflito de chave única")]
    Conflict,

    #[error("registro não encontrado")]
    NotFound,

    #[error("dados inválidos: {0}")]
    InvalidData(String),
}

/// Erros de validação de domínio.
#[derive(Debug, Error, PartialEq)]
pub enum ValidationError {
    #[error("timeframe inválido: {0}")]
    InvalidTimeFrame(String),

    #[error("candle inválido: {0}")]
    InvalidCandle(String),

    #[error("quantidade inválida: {0}")]
    InvalidQuantity(String),

    #[error("preço inválido: {0}")]
    InvalidPrice(String),

    #[error("ordem inválida: {0}")]
    InvalidOrder(String),
}
